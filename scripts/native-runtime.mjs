#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const APP = resolve(dirname(fileURLToPath(import.meta.url)), '../app');
const RECORD = resolve(APP, 'native-runtime.json');
const PLATFORM = 'android';

function currentFingerprint() {
  const output = execFileSync('npx', ['expo-updates', 'fingerprint:generate', '--platform', PLATFORM], {
    cwd: APP,
    encoding: 'utf8',
    env: { ...process.env, EXPO_PUBLIC_DISTRIBUTION: '' },
    stdio: ['ignore', 'pipe', 'ignore'],
    maxBuffer: 64 * 1024 * 1024,
  });
  return JSON.parse(output.slice(output.indexOf('{'))).hash;
}

const current = currentFingerprint();
if (process.argv.includes('--update')) {
  writeFileSync(RECORD, `${JSON.stringify({ platform: PLATFORM, fingerprint: current }, null, 2)}\n`);
  process.stdout.write(`recorded native runtime ${current}\n`);
  process.exit(0);
}

const recorded = JSON.parse(readFileSync(RECORD, 'utf8')).fingerprint;
if (current !== recorded) {
  process.stdout.write(
    `The native runtime changed: ${recorded} -> ${current}.\n` +
      'Phones on the current build will not receive this JavaScript over the air: they need a new APK and Play bundle.\n' +
      'Native dependencies, app.json, app.config.ts, the icons, the scripts block of package.json and app/.gitignore all feed it.\n' +
      'If the native change is intended, run `node scripts/native-runtime.mjs --update` and say in the PR that a native release is needed.\n',
  );
  process.exit(1);
}
process.stdout.write(`native runtime unchanged (${current})\n`);
