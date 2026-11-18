import { Linking, Platform, Share } from 'react-native';
import { showToast } from '@/lib/toast';
import { api, API_URL, historyExportUrl, type Member, type Round, type StatsPeriod, type Story } from '@/lib/api/client';
import { featuresQuery } from '@/lib/api/auth';
import { getLang, t } from '@/lib/i18n';
import { queryClient } from '@/lib/api/query-client';
import { storage } from '@/lib/device/storage';
import { askConfirm } from '@/lib/confirm';

/** Where friends can open the app (web app served by the API server). */
export const appUrl = () => API_URL || (typeof location !== 'undefined' ? location.origin : '');

/** My friend link (what the QR code holds). */
export const friendLink = (code: string) => `${appUrl()}/?friend=${code}`;

export function friendMessage(code: string) {
  return t('share.friend', { link: friendLink(code), code });
}

/** Invite to a room: the link carries the code, so it opens the join screen ready to go (signed in too: see _layout). */
export const inviteLink = (code: string) => `${appUrl()}/join?code=${code}`;

export function inviteMessage(roomName: string, code: string) {
  return t('share.invite', { room: roomName, link: inviteLink(code), code });
}

/** The first public link on this device asks first: anyone with it sees the card and the room's name. */
const PUBLIC_OK = 'qdn.publicShareOk';
export async function confirmPublic(): Promise<boolean> {
  if ((await storage.get(PUBLIC_OK).catch(() => null)) === '1') return true;
  const ok = await askConfirm(t('share.public.title'), t('share.public.message'), t('share.public.action'), 'acc');
  if (ok) storage.set(PUBLIC_OK, '1').catch(() => {});
  return ok;
}

/** One of my public links (`GET /shares`): a question (with its result or not) or a stats story. */
export type MyLink = {
  token: string;
  path: string;
  kind: 'question' | 'stat';
  title: string;
  room: string;
  with_result: boolean;
  created_at: string;
  /** Null for links made before links expired. */
  expires_at: string | null;
};
export const myLinksKey = ['shares'] as const;
export const myLinks = () => api<MyLink[]>('/shares');
export const revokeLink = (token: string) => api(`/shares/${token}`, { method: 'DELETE' });

/** Public link to a card of this question (with its podium when `result`), previewed as an image in chats. */
export async function publicLink(roundId: number, result: boolean): Promise<string | null> {
  try {
    const r = await api<{ path: string }>(`/rounds/${roundId}/share`, { method: 'POST', json: { result } });
    queryClient.invalidateQueries({ queryKey: myLinksKey });
    return `${appUrl()}${r.path}`;
  } catch {
    return null;
  }
}

/** The open question, as a message inviting people to vote. */
export function questionMessage(round: Round) {
  return t('share.question', { number: round.number, question: round.question, url: appUrl() });
}

/** A finished question with its podium. */
export function resultMessage(round: Round, byId: Map<number, Member>) {
  // A choose question: what each person really took (the first few).
  if (round.choose && round.items) {
    const items = round.items;
    const lines = round.choose.people
      .filter((p) => p.real != null)
      .slice(0, 5)
      .map((p) => `${byId.get(p.member_id)?.name ?? '?'} · ${items[p.real!] ?? '?'}`);
    return t('share.result', { number: round.number, question: round.question, lines: lines.join('\n'), url: appUrl() });
  }
  // A said question: its top scorers.
  if (round.said?.result) {
    const medals = ['🥇', '🥈', '🥉'];
    const lines = round.said.result.scores
      .filter((s) => s.points > 0)
      .slice(0, 3)
      .map((s, i) => `${medals[i]} ${byId.get(s.member_id)?.name ?? '?'} · ${t('share.points', { points: s.points })}`);
    return t('share.result', { number: round.number, question: round.question, lines: lines.join('\n'), url: appUrl() });
  }
  if (round.ranking) {
    const medals = ['🥇', '🥈', '🥉'];
    const lines = round.ranking
      .slice(0, 3)
      .map((r, i) => `${medals[i]} ${byId.get(r.member_id)?.name ?? '?'} · ${t('share.points', { points: r.points })}`);
    return t('share.ranking', { number: round.number, question: round.question, lines: lines.join('\n'), url: appUrl() });
  }
  const tallies = round.result ?? [];
  const total = tallies.reduce((n, x) => n + x.count, 0);
  const medals = ['👑', '🥈', '🥉'];
  const lines = tallies.slice(0, 3).map((x, i) => `${medals[i]} ${byId.get(x.target_id)?.name ?? '?'} · ${x.count}/${total}`);
  return t('share.result', { number: round.number, question: round.question, lines: lines.join('\n'), url: appUrl() });
}

/** Native share sheet; on the web, the browser's share or the clipboard. Returns 'copied' when it fell back to the clipboard. */
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

/**
 * Share one stats story: the server keeps a snapshot and gives a public link whose preview is the card.
 * On the web, when the browser can share files, the card itself goes out as a picture (with the link in the text).
 */
export async function shareStory(period: StatsPeriod, story: Story): Promise<'shared' | 'copied' | 'failed'> {
  return shareCard(
    () => api<{ token: string; path: string }>('/stats/share', { method: 'POST', json: { period, key: story.key, tz: -new Date().getTimezoneOffset() } }),
    t('share.story', { title: story.title, line: story.line }),
  );
}

/**
 * Share a card the server freezes into a public link (`snapshot` asks for it): asked first the very first time, then
 * the picture itself where the browser can share files, else the text with the link (the app's address if it failed).
 */
export async function shareCard(snapshot: () => Promise<{ path: string }>, text: string): Promise<'shared' | 'copied' | 'failed'> {
  if (!(await confirmPublic())) return 'failed';
  let link: string | null = null;
  try {
    const r = await snapshot();
    link = `${appUrl()}${r.path}`;
    queryClient.invalidateQueries({ queryKey: myLinksKey });
  } catch {}
  if (link && Platform.OS === 'web' && typeof navigator !== 'undefined' && navigator.canShare && typeof File !== 'undefined') {
    try {
      const blob = await (await fetch(`${link}/card.png`)).blob();
      const file = new File([blob], 'qui-de-nous.png', { type: 'image/png' });
      if (navigator.canShare({ files: [file] })) {
        await navigator.share({ files: [file], text: `${text}\n${link}` });
        return 'shared';
      }
    } catch (e) {
      // Closing the share sheet is an answer, not a failure; anything else falls back to the text.
      if (e instanceof Error && e.name === 'AbortError') return 'shared';
    }
  }
  return shareText(link ? `${text}\n${link}` : `${text}\n${appUrl()}`);
}

/** Download the history CSV: a link click on the web, the browser elsewhere. */
export async function downloadHistory() {
  // A ticket rather than the token in the URL: features fetched first if not known yet.
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

/**
 * The pack file template for an AI agent, in the app's language: a file download on the web, the share sheet with its text
 * elsewhere (to paste into an AI app). Can't fetch it: opened in the browser instead.
 */
export async function downloadPackTemplate() {
  const url = `${appUrl()}${getLang() === 'en' ? '/en' : ''}/pack-template.md`;
  try {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const text = await res.text();
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
