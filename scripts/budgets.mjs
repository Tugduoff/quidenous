#!/usr/bin/env node
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');

const BUDGETS = [
  { root: 'server/src', extension: '.rs', maxLines: 800 },
  { root: 'server/tests', extension: '.rs', maxLines: 1200 },
];

function filesUnder(directory, extension) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return filesUnder(path, extension);
    return entry.name.endsWith(extension) ? [path] : [];
  });
}

const overBudget = BUDGETS.flatMap(({ root, extension, maxLines }) =>
  filesUnder(resolve(REPO, root), extension)
    .map((file) => ({ file, lines: readFileSync(file, 'utf8').split('\n').length - 1, maxLines }))
    .filter(({ lines }) => lines > maxLines),
);

for (const { file, lines, maxLines } of overBudget) {
  process.stdout.write(`${relative(REPO, file)}: ${lines} lines (budget ${maxLines})\n`);
}
if (overBudget.length > 0) {
  process.stdout.write('\nSplit these into focused modules: a file an agent has to read whole should stay small.\n');
  process.exit(1);
}
