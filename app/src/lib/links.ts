import { appUrl } from '@/lib/api/client';
import { t } from '@/lib/i18n';
import type { Round } from '@/lib/api/rounds';
import type { Member } from '@/lib/api/group';

export const friendLink = (code: string) => `${appUrl()}/?friend=${code}`;

export function friendMessage(code: string) {
  return t('share.friend', { link: friendLink(code), code });
}

export const inviteLink = (code: string) => `${appUrl()}/join?code=${code}`;

export function inviteMessage(roomName: string, code: string) {
  return t('share.invite', { room: roomName, link: inviteLink(code), code });
}

export function questionMessage(round: Round) {
  return t('share.question', { number: round.number, question: round.question, url: appUrl() });
}

export function resultMessage(round: Round, byId: Map<number, Member>) {
  if (round.choose && round.items) {
    const items = round.items;
    const lines = round.choose.people
      .filter((p): p is typeof p & { real: number } => p.real != null)
      .slice(0, 5)
      .map((p) => `${byId.get(p.member_id)?.name ?? '?'} · ${items[p.real] ?? '?'}`);
    return t('share.result', { number: round.number, question: round.question, lines: lines.join('\n'), url: appUrl() });
  }
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

export const packLink = (id: number) => `${appUrl()}/p/${id}`;

export const partyJoinLink = (code: string) => `${appUrl()}/party/join/${code}`;

export const partyScreenLink = (code: string) => `${appUrl()}/party/screen/${code}`;
