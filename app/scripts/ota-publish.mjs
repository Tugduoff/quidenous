#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID, sign } from 'node:crypto';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { parseArgs } from 'node:util';

const { values: args } = parseArgs({
  options: {
    runtime: { type: 'string' },
    dist: { type: 'string', default: 'dist' },
    platform: { type: 'string', default: 'android' },
    url: { type: 'string', default: 'https://qdn.coflop.fr' },
    out: { type: 'string' },
    'created-at': { type: 'string' },
    'expo-config': { type: 'string' },
    'rollback-embedded': { type: 'boolean', default: false },
  },
});
const base = args.url.replace(/\/+$/, '');
const RUNTIME = /^[A-Za-z0-9._-]{1,128}$/;
if (args.runtime && !RUNTIME.test(args.runtime)) throw new Error(`bad runtime version: ${args.runtime}`);
if (!args.runtime && !args['rollback-embedded']) throw new Error('--runtime <runtime version> is required');

const CONTENT_TYPES = {
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  gif: 'image/gif',
  webp: 'image/webp',
  svg: 'image/svg+xml',
  ttf: 'font/ttf',
  otf: 'font/otf',
  xml: 'application/xml',
  json: 'application/json',
  mp3: 'audio/mpeg',
  mp4: 'video/mp4',
};
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('base64url');
const md5 = (bytes) => createHash('md5').update(bytes).digest('hex');

function token() {
  const t = process.env.UPDATES_PUBLISH_TOKEN;
  if (!t) throw new Error('UPDATES_PUBLISH_TOKEN is not set');
  return t;
}

function signature(body) {
  const key = process.env.UPDATES_PRIVATE_KEY;
  if (!key) return undefined;
  return `sig="${sign('sha256', body, key).toString('base64')}", keyid="main"`;
}

function manifest(work, runtime) {
  const files = JSON.parse(readFileSync(join(args.dist, 'metadata.json'), 'utf8')).fileMetadata?.[args.platform];
  if (!files) throw new Error(`no ${args.platform} export in ${args.dist}`);
  const id = randomUUID();
  mkdirSync(join(work, 'assets'));
  const asset = (path, ext, contentType) => {
    const bytes = readFileSync(join(args.dist, path));
    const key = md5(bytes);
    const name = `${key}.${ext}`;
    writeFileSync(join(work, 'assets', name), bytes);
    return { hash: sha256(bytes), key, contentType, fileExtension: `.${ext}`, url: `${base}/api/updates/assets/${runtime}/${id}/${name}` };
  };
  const expoClient = JSON.parse(
    args['expo-config']
      ? readFileSync(args['expo-config'], 'utf8')
      : execFileSync('npx', ['expo', 'config', '--json', '--type', 'public'], { encoding: 'utf8' }),
  );
  return {
    id,
    createdAt: new Date(args['created-at'] ?? Date.now()).toISOString(),
    runtimeVersion: runtime,
    launchAsset: asset(files.bundle, 'bundle', 'application/javascript'),
    assets: files.assets.map((a) => asset(a.path, a.ext, CONTENT_TYPES[a.ext] ?? 'application/octet-stream')),
    metadata: {},
    extra: { expoClient },
  };
}

async function publish(kind, runtime, platform, build) {
  const work = mkdtempSync(join(tmpdir(), 'ota-'));
  const archive = `${work}.tar.gz`;
  try {
    const body = Buffer.from(JSON.stringify(build(work)));
    const sig = signature(body);
    if (!sig && !args.out) throw new Error('UPDATES_PRIVATE_KEY is not set: phones refuse unsigned updates');
    if (!sig) console.warn('UPDATES_PRIVATE_KEY not set: unsigned, phones will refuse it');
    writeFileSync(join(work, 'body.json'), body);
    writeFileSync(join(work, 'update.json'), JSON.stringify({ kind, platform, runtimeVersion: runtime, ...(sig && { signature: sig }) }));
    execFileSync('tar', ['-czf', archive, '-C', work, '.']);
    if (args.out) {
      copyFileSync(archive, args.out);
      console.log(`${kind} for ${runtime} (${platform}) written to ${args.out}`);
      return;
    }
    const bytes = readFileSync(archive);
    const res = await fetch(`${base}/api/updates/publish`, {
      method: 'POST',
      headers: { authorization: `Bearer ${token()}`, 'content-type': 'application/gzip' },
      body: bytes,
    });
    const text = await res.text();
    if (!res.ok) throw new Error(`publish failed: ${res.status} ${text}`);
    console.log(`published ${kind} for ${runtime} (${platform}, ${Math.round(bytes.length / 1024)} KB): ${text}`);
  } finally {
    rmSync(work, { recursive: true, force: true });
    rmSync(archive, { force: true });
  }
}

const rollBack = () => ({ type: 'rollBackToEmbedded', parameters: { commitTime: new Date().toISOString() } });

if (!args['rollback-embedded']) {
  await publish('update', args.runtime, args.platform, (work) => manifest(work, args.runtime));
} else if (args.runtime) {
  await publish('directive', args.runtime, args.platform, rollBack);
} else {
  const res = await fetch(`${base}/api/updates`, { headers: { authorization: `Bearer ${token()}` } });
  if (!res.ok) throw new Error(`list failed: ${res.status} ${await res.text()}`);
  const live = (await res.json()).filter((u) => u.current && u.kind === 'update');
  if (!live.length) console.log('nothing to roll back');
  for (const u of live) await publish('directive', u.runtimeVersion, u.platform, rollBack);
}
