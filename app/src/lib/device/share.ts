import { Linking, Platform, Share } from 'react-native';
import { showToast } from '@/lib/toast';
import { appUrl, readUriBlob } from '@/lib/api/client';
import { getLang, t } from '@/lib/i18n';
import { queryClient } from '@/lib/api/query-client';
import { storage } from '@/lib/device/storage';
import { askConfirm } from '@/lib/confirm';
import { historyExportUrl } from '@/lib/api/history';
import type { StatsPeriod, Story } from '@/lib/api/stats';
import { featuresQuery } from '@/lib/api/features';
import { shareKeys } from '@/lib/api/share';
import { snapshotStory } from '@/lib/api/stats';
import { getPackTemplate } from '@/lib/api/packs';

const PUBLIC_OK = 'qdn.publicShareOk';
export async function confirmPublic(): Promise<boolean> {
  if ((await storage.get(PUBLIC_OK).catch(() => null)) === '1') return true;
  const ok = await askConfirm(t('share.public.title'), t('share.public.message'), t('share.public.action'), 'acc');
  if (ok) storage.set(PUBLIC_OK, '1').catch(() => {});
  return ok;
}

export async function shareText(message: string): Promise<'shared' | 'copied' | 'failed'> {
  try {
    if (Platform.OS === 'web') {
      if (typeof navigator !== 'undefined' && navigator.share) {
        await navigator.share({ text: message });
        return 'shared';
      }
      await navigator.clipboard.writeText(message);
      return 'copied';
    }
    await Share.share({ message });
    return 'shared';
  } catch {
    return 'failed';
  }
}

export async function shareStory(period: StatsPeriod, story: Story): Promise<'shared' | 'copied' | 'failed'> {
  return shareCard(
    () => snapshotStory(period, story.key),
    t('share.story', { title: story.title, line: story.line }),
  );
}

export async function shareCard(snapshot: () => Promise<{ path: string }>, text: string): Promise<'shared' | 'copied' | 'failed'> {
  if (!(await confirmPublic())) return 'failed';
  let link: string | null = null;
  try {
    const r = await snapshot();
    link = `${appUrl()}${r.path}`;
    queryClient.invalidateQueries({ queryKey: shareKeys.mine });
  } catch {}
  if (link && Platform.OS === 'web' && typeof navigator !== 'undefined' && navigator.canShare && typeof File !== 'undefined') {
    try {
      const blob = await readUriBlob(`${link}/card.png`);
      const file = new File([blob], 'qui-de-nous.png', { type: 'image/png' });
      if (navigator.canShare({ files: [file] })) {
        await navigator.share({ files: [file], text: `${text}\n${link}` });
        return 'shared';
      }
    } catch (e) {
      if (e instanceof Error && e.name === 'AbortError') return 'shared';
    }
  }
  return shareText(link ? `${text}\n${link}` : `${text}\n${appUrl()}`);
}

export async function downloadHistory() {
  const url = await queryClient
    .ensureQueryData(featuresQuery)
    .then((f) => historyExportUrl(f.includes('tickets')))
    .catch(() => null);
  if (!url) return showToast(t('common.failed'));
  if (Platform.OS === 'web' && typeof document !== 'undefined') {
    const a = document.createElement('a');
    a.href = url;
    a.download = t('share.historyFile');
    a.click();
    return;
  }
  Linking.openURL(url);
}

export async function downloadPackTemplate() {
  const url = `${appUrl()}${getLang() === 'en' ? '/en' : ''}/pack-template.md`;
  try {
    const text = await getPackTemplate(url);
    if (Platform.OS === 'web' && typeof document !== 'undefined') {
      const href = URL.createObjectURL(new Blob([text], { type: 'text/markdown' }));
      const a = document.createElement('a');
      a.href = href;
      a.download = 'quidenous-pack-template.md';
      a.click();
      setTimeout(() => URL.revokeObjectURL(href), 10_000);
      return;
    }
    await Share.share({ message: text });
  } catch {
    Linking.openURL(url).catch(() => showToast(t('common.failed')));
  }
}
