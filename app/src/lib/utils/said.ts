import type { SaidAnswer, SaidGuess, SaidResult, SaidView } from '@/lib/api/rounds';

export const ANSWER_MAX = 140;
export const ANSWER_WARN = 110;

export function toGuess(said: Pick<SaidView, 'answers' | 'my_answer'>): SaidAnswer[] {
  const mine = said.my_answer?.id;
  return (said.answers ?? []).filter((a) => a.id !== mine && (a.text != null || a.hidden));
}

export function suspects(said: Pick<SaidView, 'writer_ids'>, me: number | undefined): number[] {
  return said.writer_ids.filter((id) => id !== me);
}

export type Match = { order: number[]; picks: Record<number, number>; sel: number | null };

export type MatchAction =
  | { type: 'select'; answer: number }
  | { type: 'pick'; member: number }
  | { type: 'clear'; answer: number }
  | { type: 'reset' };

export function nextOpen(order: number[], picks: Record<number, number>, from: number | null = null): number | null {
  const start = from == null ? 0 : order.indexOf(from) + 1;
  for (let k = 0; k < order.length; k++) {
    const id = order[(start + k) % order.length];
    if (picks[id] == null) return id;
  }
  return null;
}

export function holderOf(picks: Record<number, number>, member: number): number | null {
  for (const [answer, m] of Object.entries(picks)) if (m === member) return Number(answer);
  return null;
}

export function initMatch(order: number[], picks: Record<number, number> = {}): Match {
  const kept: Record<number, number> = {};
  for (const id of order) if (picks[id] != null) kept[id] = picks[id];
  return { order, picks: kept, sel: nextOpen(order, kept) };
}

export function matchReducer(s: Match, a: MatchAction): Match {
  switch (a.type) {
    case 'select':
      if (!s.order.includes(a.answer)) return s;
      return { ...s, sel: s.sel === a.answer ? null : a.answer };
    case 'clear': {
      if (s.picks[a.answer] == null) return s;
      const picks = { ...s.picks };
      delete picks[a.answer];
      return { ...s, picks, sel: a.answer };
    }
    case 'reset':
      return initMatch(s.order);
    case 'pick': {
      const holder = holderOf(s.picks, a.member);
      if (s.sel == null) return holder == null ? s : { ...s, sel: holder };
      const target = s.sel;
      const prev = s.picks[target];
      if (prev === a.member) {
        const picks = { ...s.picks };
        delete picks[target];
        return { ...s, picks };
      }
      const picks = { ...s.picks, [target]: a.member };
      if (holder != null) {
        if (prev != null) picks[holder] = prev;
        else delete picks[holder];
      }
      return { ...s, picks, sel: nextOpen(s.order, picks, target) };
    }
  }
}

export const isComplete = (s: Match) => s.order.length > 0 && s.order.every((id) => s.picks[id] != null);
export const placed = (s: Match) => s.order.filter((id) => s.picks[id] != null).length;

export const toGuesses = (s: Match): SaidGuess[] =>
  s.order.filter((id) => s.picks[id] != null).map((id) => ({ answer_id: id, member_id: s.picks[id] }));

export type RevealItem = SaidResult['answers'][number] & {
  text: string | null;
  hidden: boolean;
  own: boolean;
  myGuess: number | null;
};

export function revealItems(said: Pick<SaidView, 'answers' | 'my_answer' | 'my_guesses' | 'result'>, me: number | undefined): RevealItem[] {
  const texts = new Map((said.answers ?? []).map((a) => [a.id, a]));
  const mine = new Map((said.my_guesses ?? []).map((g) => [g.answer_id, g.member_id]));
  return (said.result?.answers ?? []).map((r) => {
    const a = texts.get(r.id);
    return {
      ...r,
      text: a?.text ?? null,
      hidden: !!a?.hidden,
      own: r.id === said.my_answer?.id || (me != null && r.author_id === me),
      myGuess: mine.get(r.id) ?? null,
    };
  });
}

export function guessGroups(item: Pick<RevealItem, 'guesses' | 'author_id'>): { member_id: number; voters: number[]; right: boolean }[] {
  const by = new Map<number, number[]>();
  for (const g of item.guesses) by.set(g.member_id, [...(by.get(g.member_id) ?? []), g.voter_id]);
  return [...by.entries()]
    .map(([member_id, voters]) => ({ member_id, voters, right: member_id === item.author_id }))
    .sort((a, b) => b.voters.length - a.voters.length || a.member_id - b.member_id);
}

export function places(scores: { points: number }[]): number[] {
  return scores.map((s) => 1 + scores.filter((x) => x.points > s.points).length);
}

export function topScorers(scores: SaidResult['scores']): number[] {
  const best = Math.max(0, ...scores.map((s) => s.points));
  return best > 0 ? scores.filter((s) => s.points === best).map((s) => s.member_id) : [];
}

export function pickLine<T>(lines: readonly T[], seed: number): T {
  return lines[(Math.imul(seed | 0, 2654435761) >>> 0) % lines.length];
}

export const GUESS_IN = 250;
export const GUESS_STEP = 120;
export const FLIP_MS = 460;
const READ_GUESSES_MS = 650;
export const flipAt = (groups: number) => GUESS_IN + Math.max(1, groups) * GUESS_STEP + READ_GUESSES_MS;

const SCREEN_HOLD = 4500;
export const screenRevealSteps = (items: RevealItem[]) => items.map((it) => flipAt(guessGroups(it).length) + FLIP_MS + SCREEN_HOLD);

const SCREEN_GROUPS = 3;

export function screenGroups<G extends { right: boolean }>(all: G[]): { groups: G[]; rest: number } {
  let groups = all.slice(0, SCREEN_GROUPS);
  const right = all.find((g) => g.right);
  if (right && !groups.includes(right)) groups = [...groups.slice(0, SCREEN_GROUPS - 1), right];
  return { groups, rest: all.length - groups.length };
}
