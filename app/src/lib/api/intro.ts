import type { QueryClient } from '@tanstack/react-query';
import { Platform } from 'react-native';
import { api } from '@/lib/api/client';
import { INTRO_KEY, introAnswer, introPlan, introPreview, type IntroMark, readMark } from '@/lib/utils/intro';
import { storage } from '@/lib/device/storage';
import { featuresQuery } from '@/lib/api/features';

const INTRO_PREVIEW = Platform.OS === 'web' && typeof location !== 'undefined' && introPreview(location.search);

const remember = (mark: IntroMark) => storage.set(INTRO_KEY, mark).catch(() => {});

const within = <T>(p: Promise<T>, ms: number) => Promise.race([p, new Promise<null>((resolve) => setTimeout(() => resolve(null), ms))]);

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

async function tell(skipped: boolean) {
  try {
    await api('/intro', { method: 'POST', json: { skipped } });
    await remember('no');
  } catch {}
}

export async function introAtStart(qc: QueryClient): Promise<boolean> {
  if (INTRO_PREVIEW) return true;
  const mark = readMark(await storage.get(INTRO_KEY).catch(() => null));
  const plan = introPlan(mark);
  if (plan === 'resend') void tell(mark === 'skipped');
  if (plan !== 'ask') return false;
  return (await within(ask(qc), 2500)) ?? false;
}

export async function introAtSignIn(qc: QueryClient): Promise<boolean> {
  await storage.remove(INTRO_KEY).catch(() => {});
  if (INTRO_PREVIEW) return true;
  return (await within(ask(qc), 4000)) ?? false;
}

export async function introFinish(skipped: boolean) {
  if (INTRO_PREVIEW) return;
  await remember(skipped ? 'skipped' : 'done');
  await tell(skipped);
}

export const introForget = () => storage.remove(INTRO_KEY).catch(() => {});
