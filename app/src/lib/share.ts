import { Linking, Platform, Share } from 'react-native';

import { api, API_URL, historyExportUrl, type Member, type Round, type StatsPeriod, type Story } from './api';

/** Where friends can open the app (web app served by the API server). */
const appUrl = () => API_URL || (typeof location !== 'undefined' ? location.origin : '');

/** My friend link (what the QR code holds). */
export const friendLink = (code: string) => `${appUrl()}/?friend=${code}`;

export function friendMessage(code: string) {
  return `Ajoute-moi en ami sur qui de nous ? 👉 ${friendLink(code)}\n(code ami : ${code})`;
}

/** Invite to a room: the link carries the code, so it opens the join screen ready to go. */
export const inviteLink = (code: string) => `${appUrl()}/?code=${code}`;

export function inviteMessage(roomName: string, code: string) {
  return `Rejoins « ${roomName} » sur qui de nous ? 👉 ${inviteLink(code)}\n(code : ${code})`;
}

/** Public link to a card of this question (with its podium when `result`), previewed as an image in chats. */
export async function publicLink(roundId: number, result: boolean): Promise<string | null> {
  try {
    const r = await api<{ path: string }>(`/rounds/${roundId}/share`, { method: 'POST', json: { result } });
    return `${appUrl()}${r.path}`;
  } catch {
    return null;
  }
}

/** The open question, as a message inviting people to vote. */
export function questionMessage(round: Round) {
  return `qui de nous ? · question #${round.number}\n« ${round.question} »\nViens voter 👉 ${appUrl()}`;
}

/** A finished question with its podium. */
export function resultMessage(round: Round, byId: Map<number, Member>) {
  if (round.ranking) {
    const medals = ['🥇', '🥈', '🥉'];
    const lines = round.ranking.slice(0, 3).map((r, i) => `${medals[i]} ${byId.get(r.member_id)?.name ?? '?'} · ${r.points} pts`);
    return `qui de nous ? · classement #${round.number}\n« ${round.question} »\n${lines.join('\n')}\n${appUrl()}`;
  }
  const tallies = round.result ?? [];
  const total = tallies.reduce((n, t) => n + t.count, 0);
  const medals = ['👑', '🥈', '🥉'];
  const lines = tallies.slice(0, 3).map((t, i) => `${medals[i]} ${byId.get(t.target_id)?.name ?? '?'} · ${t.count}/${total}`);
  return `qui de nous ? · résultat #${round.number}\n« ${round.question} »\n${lines.join('\n')}\n${appUrl()}`;
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
  let link: string | null = null;
  try {
    const r = await api<{ token: string; path: string }>('/stats/share', {
      method: 'POST',
      json: { period, key: story.key, tz: -new Date().getTimezoneOffset() },
    });
    link = `${appUrl()}${r.path}`;
  } catch {}
  const text = `qui de nous ? · ${story.title}\n${story.line}`;
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
export function downloadHistory() {
  const url = historyExportUrl();
  if (Platform.OS === 'web' && typeof document !== 'undefined') {
    const a = document.createElement('a');
    a.href = url;
    a.download = 'qui-de-nous-historique.csv';
    a.click();
    return;
  }
  Linking.openURL(url);
}
