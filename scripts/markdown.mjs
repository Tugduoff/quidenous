#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { existsSync, lstatSync, readFileSync, readlinkSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const AGENTS_MAX_LINES = 100;

const ALLOWED = [
  /^README\.md$/,
  /^AGENTS\.md$/,
  /^CLAUDE\.md$/,
  /^deploy\/README\.md$/,
  /^store\/play\/[^/]+\.md$/,
  /^server\/assets\/pack-template\.(fr|en)\.md$/,
];

const markdownFiles = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '--', '*.md'], {
  cwd: REPO,
  encoding: 'utf8',
})
  .split('\n')
  .filter(Boolean);

const problems = markdownFiles
  .filter((file) => !ALLOWED.some((pattern) => pattern.test(file)))
  .map(
    (file) =>
      `${file}: no plans, design notes or extra READMEs in the repository. Say it in code (a test, a type, a --help text); what a person must know goes in README.md, what an agent must know and tooling cannot check goes in AGENTS.md.`,
  );

const LOCAL_LINK = /\]\((?!https?:|mailto:|#)([^)#\s]+)(?:#[^)]*)?\)/g;
for (const file of markdownFiles) {
  const text = readFileSync(join(REPO, file), 'utf8');
  for (const [, target] of text.matchAll(LOCAL_LINK)) {
    if (!existsSync(resolve(REPO, dirname(file), target))) {
      problems.push(`${file}: links to ${target}, which does not exist.`);
    }
  }
}

const agentsLines = readFileSync(join(REPO, 'AGENTS.md'), 'utf8').split('\n').length - 1;
if (agentsLines > AGENTS_MAX_LINES) {
  problems.push(
    `AGENTS.md has ${agentsLines} lines (budget ${AGENTS_MAX_LINES}). Turn a rule into a lint rule, a type or a test and delete it there.`,
  );
}

const claude = join(REPO, 'CLAUDE.md');
if (!lstatSync(claude).isSymbolicLink() || readlinkSync(claude) !== 'AGENTS.md') {
  problems.push('CLAUDE.md must be a symlink to AGENTS.md: one set of instructions for every agent.');
}

if (problems.length > 0) {
  process.stdout.write(`${problems.join('\n')}\n`);
  process.exit(1);
}
