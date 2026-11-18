#!/usr/bin/env node
import { createRequire } from 'node:module';
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, extname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');

const DEFAULT_ROOTS = [
  'app/src',
  'app/scripts',
  'app/index.ts',
  'app/app.config.ts',
  'app/eslint.config.js',
  'app/knip.config.ts',
  'server/src',
  'server/tests',
  'server/build.rs',
  'server/Cargo.toml',
  'server/clippy.toml',
  'scripts',
  'check',
  'deploy',
  'Dockerfile',
  '.github',
];

const SKIPPED_DIRS = new Set(['node_modules', 'target', 'dist', '.expo', '.git', 'android', 'ios']);
const SCRIPT_EXTENSIONS = new Set(['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs']);
const HASH_EXTENSIONS = new Set(['.sh', '.yml', '.yaml', '.toml', '.service', '.timer']);
const HASH_BASENAMES = new Set(['Dockerfile', 'check', '.dockerignore', '.gitignore', '.env.example']);

const SCRIPT_DIRECTIVES = [
  /^\/\/ eslint-disable-(next-line|line) [@\w/-]+(, [@\w/-]+)*$/,
  /^\/\* eslint-disable [@\w/-]+(, [@\w/-]+)* \*\/$/,
  /^\/\* eslint-enable( [@\w/-]+(, [@\w/-]+)*)? \*\/$/,
  /^\/\/ @ts-expect-error$/,
  /^\/\/ @generated$/,
  /^\/\/\/ <reference [^>]+\/>$/,
];
const HASH_DIRECTIVES = [
  /^#!/,
  /^# syntax=\S+$/,
  /^# shellcheck (disable|source)=\S+$/,
  /^# (v\d+(\.\d+)*|master \d{4}-\d{2}-\d{2})$/,
];
const RUST_DIRECTIVES = [/^\/\/ @generated$/];

function languageOf(file) {
  const ext = extname(file);
  if (ext === '.rs') return 'rust';
  if (SCRIPT_EXTENSIONS.has(ext)) return 'script';
  if (HASH_EXTENSIONS.has(ext) || HASH_BASENAMES.has(basename(file))) return 'hash';
  return null;
}

function filesUnder(path) {
  let stat;
  try {
    stat = statSync(path);
  } catch {
    return [];
  }
  if (stat.isFile()) return languageOf(path) ? [path] : [];
  return readdirSync(path, { withFileTypes: true })
    .filter((entry) => !SKIPPED_DIRS.has(entry.name))
    .flatMap((entry) => filesUnder(join(path, entry.name)));
}

function isAllowed(text, directives) {
  return directives.some((directive) => directive.test(text.trim()));
}

function rustComments(source) {
  const comments = [];
  const length = source.length;
  let i = 0;
  const skipString = () => {
    i += 1;
    while (i < length && source[i] !== '"') i += source[i] === '\\' ? 2 : 1;
    i += 1;
  };
  const rawStringHashes = () => {
    let j = i;
    if (source[j] === 'b' || source[j] === 'c') j += 1;
    if (source[j] !== 'r') return -1;
    j += 1;
    let hashes = 0;
    while (source[j] === '#') {
      hashes += 1;
      j += 1;
    }
    return source[j] === '"' ? hashes : -1;
  };
  const identifierBefore = () => i > 0 && /[A-Za-z0-9_]/.test(source[i - 1]);
  while (i < length) {
    const char = source[i];
    const next = source[i + 1];
    if (char === '/' && next === '/') {
      const end = source.indexOf('\n', i);
      const stop = end === -1 ? length : end;
      comments.push({ start: i, end: stop });
      i = stop;
    } else if (char === '/' && next === '*') {
      const start = i;
      let depth = 1;
      i += 2;
      while (i < length && depth > 0) {
        if (source[i] === '/' && source[i + 1] === '*') {
          depth += 1;
          i += 2;
        } else if (source[i] === '*' && source[i + 1] === '/') {
          depth -= 1;
          i += 2;
        } else {
          i += 1;
        }
      }
      comments.push({ start, end: i });
    } else if (char === '"') {
      skipString();
    } else if ((char === 'r' || char === 'b' || char === 'c') && !identifierBefore() && rawStringHashes() >= 0) {
      const hashes = rawStringHashes();
      const closing = `"${'#'.repeat(hashes)}`;
      const opened = source.indexOf('"', i);
      const closed = source.indexOf(closing, opened + 1);
      i = closed === -1 ? length : closed + closing.length;
    } else if ((char === 'b' || char === 'c') && next === '"' && !identifierBefore()) {
      i += 1;
      skipString();
    } else if (char === "'") {
      if (next === '\\') {
        i += 3;
        while (i < length && source[i] !== "'") i += 1;
        i += 1;
      } else if (source[i + 2] === "'") {
        i += 3;
      } else {
        const codePoint = source.codePointAt(i + 1);
        const width = codePoint !== undefined && codePoint > 0xffff ? 2 : 1;
        i += source[i + 1 + width] === "'" && width === 2 ? 2 + width : 1;
      }
    } else {
      i += 1;
    }
  }
  return comments.map((comment) => ({
    ...comment,
    allowed: isAllowed(source.slice(comment.start, comment.end), RUST_DIRECTIVES),
  }));
}

let typescript;
function loadTypescript() {
  if (!typescript) typescript = createRequire(join(REPO, 'app/package.json'))('typescript');
  return typescript;
}

function scriptComments(source, file) {
  const ts = loadTypescript();
  const ext = extname(file);
  const kind = ext === '.tsx' ? ts.ScriptKind.TSX : ext === '.ts' ? ts.ScriptKind.TS : ext === '.jsx' ? ts.ScriptKind.JSX : ts.ScriptKind.JS;
  const tree = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, kind);
  const found = new Map();
  const jsxText = [];
  const emptyJsxExpressions = [];
  const collect = (ranges) => {
    for (const range of ranges ?? []) found.set(range.pos, { start: range.pos, end: range.end });
  };
  const visit = (node) => {
    if (node.kind === ts.SyntaxKind.JsxText) {
      jsxText.push({ start: node.pos, end: node.end });
      return;
    }
    if (node.kind === ts.SyntaxKind.JsxExpression && !node.expression) {
      emptyJsxExpressions.push({ start: node.getStart(tree), end: node.end });
    }
    const children = node.getChildren(tree);
    if (children.length === 0) {
      collect(ts.getLeadingCommentRanges(source, node.getFullStart()));
      collect(ts.getTrailingCommentRanges(source, node.end));
    }
    children.forEach(visit);
  };
  visit(tree);
  if (source.startsWith('#!')) found.delete(0);
  const byStart = new Map();
  for (const comment of found.values()) {
    if (jsxText.some((text) => comment.start >= text.start && comment.end <= text.end)) continue;
    const wrapper = emptyJsxExpressions.find((jsx) => comment.start >= jsx.start && comment.end <= jsx.end);
    const range = wrapper ?? comment;
    const allowed = isAllowed(source.slice(comment.start, comment.end), SCRIPT_DIRECTIVES);
    const known = byStart.get(range.start);
    byStart.set(range.start, { start: range.start, end: range.end, allowed: allowed && (known?.allowed ?? true) });
  }
  return [...byStart.values()].sort((a, b) => a.start - b.start);
}

function hashCommentStart(line) {
  let quote = null;
  for (let i = 0; i < line.length; i += 1) {
    const char = line[i];
    if (quote) {
      if (char === '\\' && quote === '"') i += 1;
      else if (char === quote) quote = null;
    } else if (char === '"' || char === "'") {
      quote = char;
    } else if (char === '#' && (i === 0 || /\s/.test(line[i - 1]))) {
      return i;
    }
  }
  return -1;
}

function hashComments(source) {
  const comments = [];
  let offset = 0;
  for (const line of source.split('\n')) {
    const start = hashCommentStart(line);
    if (start !== -1) {
      const text = line.slice(start);
      comments.push({ start: offset + start, end: offset + line.length, allowed: isAllowed(text, HASH_DIRECTIVES) });
    }
    offset += line.length + 1;
  }
  return comments;
}

function commentsIn(source, file) {
  const language = languageOf(file);
  if (language === 'rust') return rustComments(source);
  if (language === 'script') return scriptComments(source, file);
  return hashComments(source);
}

function lineOf(source, index) {
  return source.slice(0, index).split('\n').length;
}

function strip(source, banned) {
  let result = source;
  for (const comment of [...banned].sort((a, b) => b.start - a.start)) {
    const lineStart = result.lastIndexOf('\n', comment.start - 1) + 1;
    const newline = result.indexOf('\n', comment.end);
    const lineEnd = newline === -1 ? result.length : newline;
    const before = result.slice(lineStart, comment.start);
    const after = result.slice(comment.end, lineEnd);
    if (before.trim() === '' && after.trim() === '') {
      result = result.slice(0, lineStart) + result.slice(newline === -1 ? result.length : newline + 1);
    } else if (after.trim() === '') {
      result = result.slice(0, lineStart) + before.trimEnd() + result.slice(lineEnd);
    } else {
      result = result.slice(0, comment.start) + result.slice(comment.end);
    }
  }
  return result.replace(/\n{3,}/g, '\n\n').replace(/^\n+/, '');
}

const args = process.argv.slice(2);
const fix = args.includes('--fix');
const roots = args.filter((arg) => !arg.startsWith('--'));
const files = (roots.length ? roots : DEFAULT_ROOTS).flatMap((root) => filesUnder(resolve(REPO, root)));

let bannedTotal = 0;
let fixedFiles = 0;
for (const file of files) {
  const source = readFileSync(file, 'utf8');
  const banned = commentsIn(source, file).filter((comment) => !comment.allowed);
  if (banned.length === 0) continue;
  bannedTotal += banned.length;
  if (fix) {
    writeFileSync(file, strip(source, banned));
    fixedFiles += 1;
    continue;
  }
  for (const comment of banned) {
    const text = source.slice(comment.start, comment.end).split('\n')[0].slice(0, 80);
    process.stdout.write(`${relative(REPO, file)}:${lineOf(source, comment.start)}: ${text}\n`);
  }
}

if (fix) {
  process.stdout.write(`removed ${bannedTotal} comments from ${fixedFiles} files\n`);
} else if (bannedTotal > 0) {
  process.stdout.write(
    `\n${bannedTotal} comments. Say it in code instead: a clearer name, a smaller function, a type, a constant or a test.\n` +
      'Only tool directives are allowed (eslint-disable-next-line <rule>, @ts-expect-error, shebangs, action version pins).\n',
  );
  process.exit(1);
}
