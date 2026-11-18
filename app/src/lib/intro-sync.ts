// The intro, device side: ask the server once per session, remember the answer, tell the server when it's done.
// The rules themselves are in lib/intro.ts.
import type { QueryClient } from '@tanstack/react-query';
import { Platform } from 'react-native';

import { api } from './api';
import { featuresQuery } from './auth';
import { INTRO_KEY, introAnswer, introPlan, introPreview, type IntroMark, readMark } from './intro';
import { storage } from './storage';

/** `?intro=1` on the web (read at load: the router drops the query). */
export const INTRO_PREVIEW = Platform.OS === 'web' && typeof location !== 'undefined' && introPreview(location.search);

const remember = (mark: IntroMark) => storage.set(INTRO_KEY, mark).catch(() => {});

/** At most `ms` of waiting: past that, no intro this time (asked again next start). */
const within = <T>(p: Promise<T>, ms: number) => Promise.race([p, new Promise<null>((resolve) => setTimeout(() => resolve(null), ms))]);

/** The server's answer for the signed-in account (features, then `/intro`); a final one is remembered. Never throws. */
async function ask(qc: QueryClient): Promise<boolean> {
  const features = await qc.fetchQuery({ ...featuresQuery, retry: false }).catch(() => null);
  const show = features?.includes('intro')
    ? await api<{ show: boolean }>('/intro')
        .then((r) => (typeof r?.show === 'boolean' ? r.show : null))
        .catch(() => null)
    : null;
  const answer = introAnswer(features, show);
  if (answer.mark) await remember(answer.mark);
  return answer.show;
}

/** Tell the server it's done (idempotent); then nothing is left to tell. */
async function tell(skipped: boolean) {
  try {
    await api('/intro', { method: 'POST', json: { skipped } });
    await remember('no');
  } catch {}
}

/**
 * App start, signed in: show the intro? A session that already has its answer asks nothing (an existing account never
 * waits on `/intro`); otherwise the question runs alongside `/me` and gives up after 2.5 s.
 */
export async function introAtStart(qc: QueryClient): Promise<boolean> {
  if (INTRO_PREVIEW) return true;
  const mark = readMark(await storage.get(INTRO_KEY).catch(() => null));
  const plan = introPlan(mark);
  if (plan === 'resend') void tell(mark === 'skipped');
  if (plan !== 'ask') return false;
  return (await within(ask(qc), 2500)) ?? false;
}

/** Just signed in (or up): a new session, asked afresh (alongside joining the invite's room and `/me`). */
export async function introAtSignIn(qc: QueryClient): Promise<boolean> {
  await storage.remove(INTRO_KEY).catch(() => {});
  if (INTRO_PREVIEW) return true;
  return (await within(ask(qc), 4000)) ?? false;
}

/** Finished or skipped: never again on this device, even if telling the server fails now (retried at the next start). */
export async function introFinish(skipped: boolean) {
  if (INTRO_PREVIEW) return;
  await remember(skipped ? 'skipped' : 'done');
  await tell(skipped);
}

/** Signed out: the next account on this device is asked afresh. */
export const introForget = () => storage.remove(INTRO_KEY).catch(() => {});
