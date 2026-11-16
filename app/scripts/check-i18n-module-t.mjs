#!/usr/bin/env node
// Fails when a translation is computed at module load in app/src (docs/I18N.md): a constant made with `t(...)`
// when a file is imported stays in the first language after a switch.
// Flags calls to `t`, `dateFormat`, `formatNumber`, `intlLocale` and `getLang` imported from lib/i18n (whatever their
// local name) that run at load: at the top level, in a static class field or block, in a function called right away
// (IIFE) or passed to an array method or Object.fromEntries there. A call inside any other function is fine.
//
// Usage: node scripts/check-i18n-module-t.mjs [dir ...]   (default: src, from app/)
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

import ts from 'typescript';

const WATCHED = new Set(['t', 'dateFormat', 'formatNumber', 'intlLocale', 'getLang']);
// Callbacks these run on the spot (at load when called at the top level).
const EAGER = new Set([
  'map',
  'flatMap',
  'filter',
  'forEach',
  'reduce',
  'reduceRight',
  'some',
  'every',
  'find',
  'findIndex',
  'sort',
  'fromEntries',
  'from',
]);

const isI18n = (spec) => /(^|\/)i18n$/.test(spec);

function files(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      if (name !== 'node_modules') out.push(...files(p));
    } else if (/\.(ts|tsx|mjs|js)$/.test(name) && !name.endsWith('.d.ts')) out.push(p);
  }
  return out;
}

/** Local names bound to the watched exports of lib/i18n (`import { t as tr }` → `tr`), and namespace imports. */
function bindings(sf, self) {
  const names = new Map();
  const spaces = new Set();
  // lib/i18n itself: its own top-level functions.
  if (self) for (const w of WATCHED) names.set(w, w);
  for (const st of sf.statements) {
    if (!ts.isImportDeclaration(st) || !ts.isStringLiteral(st.moduleSpecifier) || !isI18n(st.moduleSpecifier.text)) continue;
    const b = st.importClause?.namedBindings;
    if (!b) continue;
    if (ts.isNamespaceImport(b)) spaces.add(b.name.text);
    else
      for (const el of b.elements) {
        const imported = (el.propertyName ?? el.name).text;
        if (WATCHED.has(imported)) names.set(el.name.text, imported);
      }
  }
  return { names, spaces };
}

const isFunction = (n) =>
  ts.isFunctionDeclaration(n) ||
  ts.isFunctionExpression(n) ||
  ts.isArrowFunction(n) ||
  ts.isMethodDeclaration(n) ||
  ts.isConstructorDeclaration(n) ||
  ts.isGetAccessorDeclaration(n) ||
  ts.isSetAccessorDeclaration(n);

/** A function that runs as soon as the code around it does: `(() => …)()`, `list.map((x) => …)`. */
function runsAtOnce(fn) {
  let p = fn.parent;
  while (p && ts.isParenthesizedExpression(p)) p = p.parent;
  if (!p || !ts.isCallExpression(p)) return false;
  if (p.expression === fn || (ts.isParenthesizedExpression(p.expression) && p.expression.expression === fn)) return true;
  if (!p.arguments.includes(fn)) return false;
  const callee = p.expression;
  return ts.isPropertyAccessExpression(callee) && EAGER.has(callee.name.text);
}

/** Whether `node` runs when the module is loaded. */
function atLoad(node) {
  for (let n = node.parent; n; n = n.parent) {
    if (isFunction(n)) {
      if (!runsAtOnce(n)) return false;
      continue;
    }
    // Instance fields run per instance; static ones and static blocks at load.
    if (ts.isPropertyDeclaration(n) && !n.modifiers?.some((m) => m.kind === ts.SyntaxKind.StaticKeyword)) return false;
  }
  return true;
}

const problems = [];
const dirs = process.argv.slice(2);
for (const dir of dirs.length ? dirs : ['src']) {
  for (const file of files(dir)) {
    const text = readFileSync(file, 'utf8');
    const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, file.endsWith('x') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
    const { names, spaces } = bindings(sf, /(^|\/)lib\/i18n\.ts$/.test(file.replaceAll('\\', '/')));
    if (!names.size && !spaces.size) continue;
    const visit = (node) => {
      if (ts.isCallExpression(node)) {
        const c = node.expression;
        const name = ts.isIdentifier(c)
          ? names.get(c.text)
          : ts.isPropertyAccessExpression(c) && ts.isIdentifier(c.expression) && spaces.has(c.expression.text) && WATCHED.has(c.name.text)
            ? c.name.text
            : undefined;
        if (name && atLoad(node)) {
          const { line, character } = sf.getLineAndCharacterOfPosition(node.getStart(sf));
          problems.push({ file: relative(process.cwd(), file), line: line + 1, col: character + 1, name });
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(sf);
  }
}

for (const p of problems) {
  const msg = `${p.name}() runs when the module loads: its text stays in the first language. Call it in the component or function that uses it (docs/I18N.md).`;
  if (process.env.GITHUB_ACTIONS) console.log(`::error file=app/${p.file},line=${p.line},col=${p.col}::${msg}`);
  console.log(`${p.file}:${p.line}:${p.col}  ${msg}`);
}
if (problems.length) {
  console.log(`\n${problems.length} translation(s) computed at module load.`);
  process.exit(1);
}
console.log('i18n: no translation at module load.');
