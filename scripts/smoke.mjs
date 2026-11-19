#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { appendFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const OUT = process.env.SMOKE_OUT ?? '/tmp/qdn-smoke';
const SERVER_BIN = process.env.SMOKE_SERVER_BIN;
const WEB_DIR = process.env.SMOKE_WEB_DIR;
const PLAYWRIGHT = process.env.PLAYWRIGHT_DIR
  ? join(process.env.PLAYWRIGHT_DIR, 'node_modules/playwright/index.mjs')
  : 'playwright';
const PASSWORD = 'Smoke-test-passw0rd';
const ROOM_NAME = 'Les testeurs';
const QUESTION = 'Qui de nous casserait la prod un vendredi soir ?';
const CHAT_MESSAGE = 'Message du test de fumée';
const STEP_TIMEOUT_MS = 20_000;
const SETTLE_MS = 1_500;
const SERVER_START_TIMEOUT_MS = 60_000;
const PHONE = { width: 390, height: 844 };
const OTHER_SCREENS = [
  '/questions',
  '/stats',
  '/group',
  '/rooms',
  '/friends',
  '/inbox',
  '/profile',
  '/security',
  '/notifications',
  '/avatar',
  '/badges',
  '/packs',
  '/packs-mine',
  '/pack/edit',
  '/suggest',
  '/global',
  '/party/new',
  '/wrapped',
];

let serverLog = '';
let clientIp = 0;

function fail(message) {
  throw new Error(message);
}

async function freePort() {
  return new Promise((resolve, reject) => {
    const probe = createServer();
    probe.once('error', reject);
    probe.listen(0, '127.0.0.1', () => {
      const { port } = probe.address();
      probe.close(() => resolve(port));
    });
  });
}

async function startServer() {
  if (!SERVER_BIN || !WEB_DIR) fail('SMOKE_SERVER_BIN and SMOKE_WEB_DIR are required (run ./check smoke)');
  const port = await freePort();
  const dataDir = mkdtempSync(join(tmpdir(), 'qdn-smoke-data-'));
  const child = spawn(SERVER_BIN, [], {
    env: {
      PATH: process.env.PATH,
      PORT: String(port),
      DATA_DIR: dataDir,
      WEB_DIR,
      JOIN_CODE: 'SMOKE-1',
      GROUP_NAME: 'Smoke',
      TRUST_PROXY: '1',
      NO_COLOR: '1',
      RUST_LOG: 'info,quidenous_server=debug',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const record = (chunk) => {
    serverLog += chunk;
    appendFileSync(join(OUT, 'server.log'), chunk);
  };
  child.stdout.on('data', record);
  child.stderr.on('data', record);
  const url = `http://127.0.0.1:${port}`;
  const deadline = Date.now() + SERVER_START_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) fail(`the server exited with code ${child.exitCode}: see ${OUT}/server.log`);
    const healthy = await fetch(`${url}/api/health`).then((res) => res.ok, () => false);
    if (healthy) return { url, stop: () => stopServer(child, dataDir) };
    await new Promise((done) => setTimeout(done, 250));
  }
  child.kill('SIGKILL');
  return fail(`the server did not answer /api/health within ${SERVER_START_TIMEOUT_MS} ms`);
}

async function stopServer(child, dataDir) {
  child.kill('SIGTERM');
  await new Promise((done) => child.once('exit', done));
  rmSync(dataDir, { recursive: true, force: true });
}

function emailedCode(email) {
  const marker = `to=${email}`;
  const at = serverLog.lastIndexOf(marker);
  if (at === -1) fail(`no e-mail to ${email} in the server log`);
  const code = serverLog.slice(at).match(/^\s*(\d{6})\s*$/m);
  return code ? code[1] : fail(`no 6-digit code in the e-mail to ${email}`);
}

function apiClient(baseUrl) {
  return async (method, path, { token, room, body } = {}) => {
    clientIp += 1;
    const headers = { 'content-type': 'application/json', 'x-forwarded-for': `10.9.${clientIp >> 8}.${clientIp & 255}` };
    if (token) headers.authorization = `Bearer ${token}`;
    if (room) headers['x-room'] = String(room);
    const res = await fetch(`${baseUrl}/api${path}`, { method, headers, body: body ? JSON.stringify(body) : undefined });
    const text = await res.text();
    if (!res.ok) fail(`${method} ${path} answered ${res.status}: ${text.slice(0, 300)}`);
    return text ? JSON.parse(text) : null;
  };
}

async function createAccount(api, name) {
  const email = `${name.toLowerCase()}@smoke.test`;
  const { token } = await api('POST', '/auth/signup', { body: { email, password: PASSWORD, name, locale: 'fr' } });
  await api('POST', '/auth/verify', { token, body: { code: emailedCode(email) } });
  await api('POST', '/auth/terms', { token, body: {} });
  await api('POST', '/intro', { token, body: {} });
  return { name, email, token };
}

async function seedRoom(api) {
  const alice = await createAccount(api, 'Alice');
  const bob = await createAccount(api, 'Bob');
  const chloe = await createAccount(api, 'Chloe');
  const room = await api('POST', '/rooms', { token: alice.token, body: { name: ROOM_NAME } });
  await api('POST', '/questions', { token: alice.token, room: room.id, body: { text: QUESTION } });
  const { code } = await api('GET', '/group', { token: alice.token, room: room.id });
  for (const friend of [bob, chloe]) await api('POST', '/rooms/join', { token: friend.token, body: { code } });
  const group = await api('GET', '/group', { token: alice.token, room: room.id });
  const memberId = (name) => group.members.find((member) => member.name === name)?.id ?? fail(`${name} is not in the room`);
  const round = await api('GET', '/round/current', { token: alice.token, room: room.id });
  if (!round) fail('the seeded room has no open question');
  if (round.question !== QUESTION) fail(`the room opened with "${round.question}" instead of its own question`);
  await api('POST', '/round/current/vote', { token: bob.token, room: room.id, body: { target_id: memberId('Alice') } });
  await api('POST', '/round/current/vote', { token: chloe.token, room: room.id, body: { target_id: memberId('Bob') } });
  return { alice };
}

async function run() {
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, 'server.log'), '');
  const { chromium } = await import(PLAYWRIGHT);
  const server = await startServer();
  const browser = await chromium.launch();
  const problems = [];
  try {
    const api = apiClient(server.url);
    const { alice } = await seedRoom(api);
    const context = await browser.newContext({ viewport: PHONE, colorScheme: 'dark', locale: 'fr-FR' });
    await context.addInitScript(() => {
      Object.defineProperty(navigator, 'standalone', { get: () => true });
    });
    const page = await context.newPage();
    page.setDefaultTimeout(STEP_TIMEOUT_MS);
    let step = 'sign-in';
    const problem = (what) => problems.push(`${step}: ${what}`);
    page.on('pageerror', (error) => problem(`page error: ${error.message}`));
    page.on('console', (message) => {
      if (message.type() === 'error') problem(`console error: ${message.text()}`);
    });
    page.on('request', (req) => {
      if (req.url().endsWith('/api/client-errors')) problem(`the app showed its crash screen: ${req.postData()}`);
    });
    page.on('response', (res) => {
      if (res.status() >= 500) problem(`HTTP ${res.status()} ${res.request().method()} ${res.url()}`);
    });
    const settle = () => page.waitForTimeout(SETTLE_MS);
    const shot = async (name) => {
      step = name;
      await settle();
      await page.screenshot({ path: join(OUT, `${name}.png`) });
    };
    const onScreen = (locator) => locator.locator('visible=true').first();
    const button = (name) => onScreen(page.getByRole('button', { name, exact: true }));
    const tab = (name) => onScreen(page.getByRole('tab', { name, exact: true }));
    const see = (text) => onScreen(page.getByText(text)).waitFor();

    await page.goto(server.url);
    await button('Continuer avec un e-mail').click();
    await onScreen(page.getByLabel('E-mail')).fill(alice.email);
    await page.keyboard.press('Enter');
    await onScreen(page.getByLabel('Mot de passe')).fill(PASSWORD);
    await page.keyboard.press('Enter');
    await see(QUESTION);
    await shot('01-vote');

    await onScreen(page.getByText('Bob', { exact: true })).click();
    await button('Voter pour Bob').click();
    await see('67%');
    await shot('02-result');

    const historyEntry = onScreen(page.getByRole('button', { name: QUESTION }));
    await tab('Historique').click();
    await historyEntry.waitFor();
    await shot('03-history');

    await historyEntry.click();
    await page.waitForURL(/\/round\/\d+$/);
    await onScreen(page.getByLabel('Écrire un message')).fill(CHAT_MESSAGE);
    await button('Envoyer').click();
    await see(CHAT_MESSAGE);
    await shot('04-chat');

    await button('Retour').click();
    await tab('Vote').click();
    await button('Groupe').click();
    await see('Code d\'invitation');
    await button('Paramètres du salon').click();
    await page.waitForURL(/\/room-settings$/);
    await see('Nouveau code d\'invitation');
    await shot('05-room-settings');

    for (const route of OTHER_SCREENS) {
      step = route;
      await page.goto(server.url + route);
      await page.waitForLoadState('networkidle');
      await shot(`screen${route.replaceAll('/', '-')}`);
      const landed = new URL(page.url()).pathname;
      if (landed !== route) problem(`the app left this screen for ${landed}`);
    }

    step = 'room theme';
    await page.goto(`${server.url}/room-settings`);
    const saved = page.waitForResponse((res) => res.request().method() === 'PATCH' && /\/api\/rooms\/\d+$/.test(res.url()));
    await onScreen(page.getByRole('radio', { name: /^Thème Piscine,/ })).click();
    if (!(await saved).ok()) problem('the room theme was not saved');
    await shot('06-room-theme');
    await page.goto(server.url);
    await see('67%');
    await shot('07-themed-vote');
  } finally {
    await browser.close();
    await server.stop();
  }
  if (problems.length > 0) fail(`the app ran with errors:\n${problems.join('\n')}`);
  process.stdout.write(`smoke run passed: screenshots in ${OUT}\n`);
}

run().catch((error) => {
  process.stderr.write(`smoke run failed: ${error.message}\n`);
  process.exit(1);
});
