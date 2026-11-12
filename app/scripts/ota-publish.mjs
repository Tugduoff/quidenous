#!/usr/bin/env node
// Over-the-air updates (expo-updates protocol v1) on our own server. See docs/OTA.md.
//
//   node scripts/ota-publish.mjs --runtime <rv> [--dist dist] [--platform android] [--created-at <ISO>] [--out update.tar.gz]
//       publishes `npx expo export` output (dist/) for the builds whose runtime is <rv>
//   node scripts/ota-publish.mjs --rollback-embedded [--runtime <rv>]
//       phones go back to the bundle inside their APK (every runtime the server serves, unless --runtime)
//   --url https://qdn.coflop.fr (default)
//
// The manifest is built and signed here (CI): the private key never reaches the server, which stores and serves the
// signed bytes as they are. Env: UPDATES_PUBLISH_TOKEN (required unless --out), UPDATES_PRIVATE_KEY (PEM; without it
// the update is unsigned, and phones, which hold the code signing certificate, refuse it).
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
    // The build's embedded bundle time: a phone running that very build doesn't download the same JS again.
    'created-at': { type: 'string' },
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

/** `expo-signature` header value for these exact bytes (RSA PKCS#1 v1.5, SHA-256), or undefined without a key. */
function signature(body) {
  const key = process.env.UPDATES_PRIVATE_KEY;
  if (!key) return undefined;
  return `sig="${sign('sha256', body, key).toString('base64')}", keyid="main"`;
}

/** The manifest of `dist/` for this runtime, its asset files copied into `work/assets/`. */
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
  // Constants.expoConfig inside the update (version, extra…): the public app config, resolved with the same env as the build.
  const expoClient = JSON.parse(execFileSync('npx', ['expo', 'config', '--json', '--type', 'public'], { encoding: 'utf8' }));
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

/** Packs update.json + body.json (+ assets/) and uploads it, or writes it to --out. */
async function publish(kind, runtime, platform, build) {
  const work = mkdtempSync(join(tmpdir(), 'ota-'));
  const archive = `${work}.tar.gz`;
  try {
    const body = Buffer.from(JSON.stringify(build(work)));
    const sig = signature(body);
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
  // Every runtime + platform whose phones currently get an update.
  const res = await fetch(`${base}/api/updates`, { headers: { authorization: `Bearer ${token()}` } });
  if (!res.ok) throw new Error(`list failed: ${res.status} ${await res.text()}`);
  const live = (await res.json()).filter((u) => u.current && u.kind === 'update');
  if (!live.length) console.log('nothing to roll back');
  for (const u of live) await publish('directive', u.runtimeVersion, u.platform, rollBack);
}
