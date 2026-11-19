#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { appendFileSync, existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const MIGRATIONS = join(REPO, 'server/migrations');
const LOCK = join(REPO, 'server/migrations.lock');
const LOCK_COMMAND = 'node scripts/migrations.mjs lock';

const sha256 = (file) => createHash('sha256').update(readFileSync(join(MIGRATIONS, file))).digest('hex');
const numberOf = (file) => file.split('_')[0];

const files = readdirSync(MIGRATIONS)
  .filter((file) => file.endsWith('.sql'))
  .sort();
const locked = new Map(
  (existsSync(LOCK) ? readFileSync(LOCK, 'utf8') : '')
    .split('\n')
    .filter(Boolean)
    .map((line) => line.split('  '))
    .map(([hash, file]) => [file, hash]),
);
const unlocked = files.filter((file) => !locked.has(file));

if (process.argv[2] === 'lock') {
  for (const file of unlocked) appendFileSync(LOCK, `${sha256(file)}  ${file}\n`);
  process.stdout.write(`${unlocked.length} new migrations recorded in server/migrations.lock\n`);
  process.exit(0);
}

const problems = [];
for (const [file, hash] of locked) {
  if (!files.includes(file)) {
    problems.push(`${file} was deleted or renamed. Production already ran it: bring it back as it was.`);
  } else if (sha256(file) !== hash) {
    problems.push(
      `${file} was edited. A migration that already ran can never change (sqlx compares checksums and production would refuse to start): put the change in a new migration. Only if this file has never reached main: delete its line in server/migrations.lock, then run ${LOCK_COMMAND}.`,
    );
  }
}
for (const file of unlocked) {
  problems.push(`${file} is new: record it with ${LOCK_COMMAND}.`);
}
const numbers = files.map(numberOf);
for (const number of new Set(numbers.filter((number, index) => numbers.indexOf(number) !== index))) {
  problems.push(`Two migrations are numbered ${number}: give the new one the next free number.`);
}

if (problems.length > 0) {
  process.stdout.write(`${problems.join('\n')}\n`);
  process.exit(1);
}
