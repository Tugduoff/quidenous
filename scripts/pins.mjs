#!/usr/bin/env node
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const read = (path) => readFileSync(join(REPO, path), 'utf8');
const all = (text, pattern) => [...text.matchAll(pattern)].map((match) => match[1]);

const workflows = readdirSync(join(REPO, '.github/workflows')).map((file) => `.github/workflows/${file}`);
const dockerfile = read('Dockerfile');

const rust = [
  ['Dockerfile', all(dockerfile, /^FROM rust:([\d.]+)-/gm)],
  ...workflows.map((file) => [file, all(read(file), /^\s+toolchain: ([\d.]+)$/gm)]),
];
const node = [
  ['Dockerfile', all(dockerfile, /^FROM node:(\d+)\./gm)],
  ...workflows.map((file) => [file, all(read(file), /^\s+node-version: (\d+)$/gm)]),
];

function disagreements(name, pins) {
  const versions = new Set(pins.flatMap(([, found]) => found));
  if (versions.size <= 1) return [];
  return [`${name} is pinned to different versions:`, ...pins.filter(([, found]) => found.length).map(([file, found]) => `  ${file}: ${[...new Set(found)].join(', ')}`)];
}

const problems = [...disagreements('Rust', rust), ...disagreements('Node', node)];
if (problems.length > 0) {
  process.stdout.write(`${problems.join('\n')}\nBump them together: CI, the Docker image and local builds must use the same toolchain.\n`);
  process.exit(1);
}
