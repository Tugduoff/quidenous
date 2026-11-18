// « Qui a dit ça ? » (docs/SAID.md): the pure part. What there is to guess, the matching game's state, the reveal.
// No React Native in here (tested with `bun test src/lib/said.test.ts`).
import type { SaidAnswer, SaidGuess, SaidResult, SaidView } from '@/lib/api';

/** Same limit as the server (1 to 140 characters after cleaning). */
export const ANSWER_MAX = 140;
/** The counter shows from here on. */
export const ANSWER_WARN = 110;

/** Answers I have to place: every answer but mine, and no pass from an older app (text null and not hidden). */
export function toGuess(said: Pick<SaidView, 'answers' | 'my_answer'>): SaidAnswer[] {
  const mine = said.my_answer?.id;
  return (said.answers ?? []).filter((a) => a.id !== mine && (a.text != null || a.hidden));
}

/** Who I can place: the writers but me, in member order. */
export function suspects(said: Pick<SaidView, 'writer_ids'>, me: number | undefined): number[] {
  return said.writer_ids.filter((id) => id !== me);
}

// ---------------------------------------------------------------------------------------------------------------------
// The matching game

/** `order`: the answers to place (ids, as shown). `picks`: answer → member. `sel`: the answer waiting for a person. */
export type Match = { order: number[]; picks: Record<number, number>; sel: number | null };

export type MatchAction =
  /** Tap an answer: it waits for a person (tap it again: not any more). */
  | { type: 'select'; answer: number }
  /** Tap a person: on the waiting answer (a person already used elsewhere swaps places with what was there). */
  | { type: 'pick'; member: number }
  /** Take the person off an answer. */
  | { type: 'clear'; answer: number }
  | { type: 'reset' };

/** The first answer with nobody on it, starting after `from` and going round (null: all placed). */
export function nextOpen(order: number[], picks: Record<number, number>, from: number | null = null): number | null {
  const start = from == null ? 0 : order.indexOf(from) + 1;
  for (let k = 0; k < order.length; k++) {
    const id = order[(start + k) % order.length];
    if (picks[id] == null) return id;
  }
  return null;
}

/** The answer a person is on (null: none). */
export function holderOf(picks: Record<number, number>, member: number): number | null {
  for (const [answer, m] of Object.entries(picks)) if (m === member) return Number(answer);
  return null;
}

/** A new game: the first open answer waits for a person, so tapping faces one after the other just works. */
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
      // Nothing waiting: tapping someone already placed shows where they are.
      if (s.sel == null) return holder == null ? s : { ...s, sel: holder };
      const target = s.sel;
      const prev = s.picks[target];
      // The same face again on its own answer: taken off.
      if (prev === a.member) {
        const picks = { ...s.picks };
        delete picks[target];
        return { ...s, picks };
      }
      const picks = { ...s.picks, [target]: a.member };
      // They were on another answer: whoever was here goes there (a swap), or it is left open (a move).
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

/** What the vote sends: one guess per answer, in the order shown. */
export const toGuesses = (s: Match): SaidGuess[] =>
  s.order.filter((id) => s.picks[id] != null).map((id) => ({ answer_id: id, member_id: s.picks[id] }));

// ---------------------------------------------------------------------------------------------------------------------
// The reveal

export type RevealItem = SaidResult['answers'][number] & {
  text: string | null;
  hidden: boolean;
  /** I wrote it. */
  own: boolean;
  /** Who I said wrote it (null: I didn't guess it, mine or I didn't play). */
  myGuess: number | null;
};

/** The result's answers in reveal order (the server's: most found first, best hidden last), with their text and my guess. */
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

/** Who was named for an answer: one group per person named, most named first; `right` once the author is known. */
export function guessGroups(item: Pick<RevealItem, 'guesses' | 'author_id'>): { member_id: number; voters: number[]; right: boolean }[] {
  const by = new Map<number, number[]>();
  for (const g of item.guesses) by.set(g.member_id, [...(by.get(g.member_id) ?? []), g.voter_id]);
  return [...by.entries()]
    .map(([member_id, voters]) => ({ member_id, voters, right: member_id === item.author_id }))
    .sort((a, b) => b.voters.length - a.voters.length || a.member_id - b.member_id);
}

/** Places in the scoreboard (best first): the same points share a place (1, 1, 3). */
export function places(scores: { points: number }[]): number[] {
  return scores.map((s) => 1 + scores.filter((x) => x.points > s.points).length);
}

/** The top scorers (points > 0; ties share): the winners of a said question. */
export function topScorers(scores: SaidResult['scores']): number[] {
  const best = Math.max(0, ...scores.map((s) => s.points));
  return best > 0 ? scores.filter((s) => s.points === best).map((s) => s.member_id) : [];
}

/** A line among several for this round (stable for a round and a step, so it doesn't flicker on re-render). */
export function pickLine<T>(lines: readonly T[], seed: number): T {
  return lines[(Math.imul(seed | 0, 2654435761) >>> 0) % lines.length];
}
