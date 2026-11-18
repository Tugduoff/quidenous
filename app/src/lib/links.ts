import { appUrl } from '@/lib/api/client';
import { t } from '@/lib/i18n';
import type { Round } from '@/lib/api/rounds';
import type { Member } from '@/lib/api/group';

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

/** The pack's public page (preview card in chats, "install in a room" button). */
export const packLink = (id: number) => `${appUrl()}/p/${id}`;

/** The link people scan or tap to join. */
export const partyJoinLink = (code: string) => `${appUrl()}/party/join/${code}`;

/** The big-screen view of a party (a TV, a laptop). */
export const partyScreenLink = (code: string) => `${appUrl()}/party/screen/${code}`;
