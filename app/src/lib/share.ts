import { Linking, Platform, Share } from 'react-native';

import { API_URL, historyExportUrl, type Member, type Round } from './api';

/** Where friends can open the app (web app served by the API server). */
const appUrl = () => API_URL || (typeof location !== 'undefined' ? location.origin : '');

/** Invite to a room: the link carries the code, so it opens the join screen ready to go. */
export function inviteMessage(roomName: string, code: string) {
  return `Rejoins « ${roomName} » sur qui de nous ? 👉 ${appUrl()}/?code=${code}\n(code : ${code})`;
}

/** The open question, as a message inviting people to vote. */
export function questionMessage(round: Round) {
  return `qui de nous ? · question #${round.number}\n« ${round.question} »\nViens voter 👉 ${appUrl()}`;
}

/** A finished question with its podium. */
export function resultMessage(round: Round, byId: Map<number, Member>) {
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
