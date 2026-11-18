import * as Notifications from 'expo-notifications';
import { Platform } from 'react-native';

import type { Lang } from '@/lib/i18n';
import { ROOM_KEY, SERVER_KEY, storage, TOKEN_KEY } from '@/lib/device/storage';

export type WidgetRound = { id: number; number: number; question: string; kind: 'pick' | 'rank' | 'choose' };
export type WidgetState = {
  state: 'result' | 'vote' | 'wait' | 'idle' | 'no_room';
  lang: Lang;
  room: { id: number; name: string; kind: 'room' | 'party' } | null;
  round: WidgetRound | null;
  voted: boolean;
  votes: number;
  needed: number;
  result: WidgetRound | null;
  unseen: number;
  streak: { current: number; today: boolean } | null;
  next_open_at: string | null;
};

export type WidgetView = { kind: 'out' } | { kind: 'loading' } | { kind: 'state'; state: WidgetState; at: number; stale?: boolean };

const WIDGET_TOKEN_KEY = 'qdn.widget.token';
const CACHE_KEY = 'qdn.widget.cache';
const TIMEOUT_MS = 10_000;

const sessionTag = (session: string) => session.slice(-8);

async function fetchWithin(url: string, init: RequestInit) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    return await fetch(url, { ...init, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}

async function serverUrl() {
  return ((await storage.get(SERVER_KEY)) || process.env.EXPO_PUBLIC_API_URL || '').replace(/\/+$/, '');
}

async function fcmToken() {
  if (Platform.OS !== 'android') return undefined;
  try {
    const { data } = await Notifications.getDevicePushTokenAsync();
    return typeof data === 'string' ? data : undefined;
  } catch {
    return undefined;
  }
}

async function mint(base: string, session: string): Promise<string | 'out' | null> {
  const res = await fetchWithin(`${base}/api/widget/token`, {
    method: 'POST',
    headers: { authorization: `Bearer ${session}`, 'content-type': 'application/json' },
    body: JSON.stringify({ fcm_token: await fcmToken() }),
  }).catch(() => null);
  if (!res) return null;
  if (res.status === 401) return 'out';
  if (!res.ok) return null;
  const { token } = (await res.json()) as { token: string };
  await storage.set(WIDGET_TOKEN_KEY, JSON.stringify({ token, of: sessionTag(session) }));
  return token;
}

async function savedToken(session: string) {
  try {
    const saved = JSON.parse((await storage.get(WIDGET_TOKEN_KEY)) ?? 'null') as { token: string; of: string } | null;
    return saved && saved.of === sessionTag(session) ? saved.token : null;
  } catch {
    return null;
  }
}

async function cached(): Promise<WidgetView> {
  try {
    const c = JSON.parse((await storage.get(CACHE_KEY)) ?? 'null') as { state: WidgetState; at: number } | null;
    if (c?.state) return { kind: 'state', state: c.state, at: c.at, stale: true };
  } catch {}
  return { kind: 'loading' };
}

export async function loadWidgetView(): Promise<WidgetView> {
  const session = await storage.get(TOKEN_KEY).catch(() => null);
  if (!session) {
    await storage.remove(CACHE_KEY).catch(() => {});
    return { kind: 'out' };
  }
  const base = await serverUrl();
  const room = Number(await storage.get(ROOM_KEY).catch(() => null)) || null;
  let token = await savedToken(session);
  for (let attempt = 0; attempt < 2; attempt++) {
    if (!token) {
      const minted = await mint(base, session);
      if (minted === 'out') {
        await storage.remove(CACHE_KEY).catch(() => {});
        return { kind: 'out' };
      }
      if (!minted) break;
      token = minted;
    }
    const res = await fetchWithin(`${base}/api/widget${room ? `?room=${room}` : ''}`, {
      headers: { authorization: `Bearer ${token}` },
    }).catch(() => null);
    if (!res) break;
    if (res.status === 401) {
      await storage.remove(WIDGET_TOKEN_KEY).catch(() => {});
      token = null;
      continue;
    }
    if (!res.ok) break;
    const state = (await res.json()) as WidgetState;
    const at = Date.now();
    await storage.set(CACHE_KEY, JSON.stringify({ state, at })).catch(() => {});
    return { kind: 'state', state, at };
  }
  return cached();
}

export async function forgetWidgetToken() {
  const raw = await storage.get(WIDGET_TOKEN_KEY).catch(() => null);
  await storage.remove(WIDGET_TOKEN_KEY).catch(() => {});
  await storage.remove(CACHE_KEY).catch(() => {});
  try {
    const saved = JSON.parse(raw ?? 'null') as { token: string } | null;
    if (saved?.token) {
      const base = await serverUrl();
      await fetchWithin(`${base}/api/widget/token`, { method: 'DELETE', headers: { authorization: `Bearer ${saved.token}` } });
    }
  } catch {}
}
