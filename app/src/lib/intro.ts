// The first-run intro (app/intro.tsx): when to show it, and the demo round the Petons play in it.
// Plain logic only (no React Native import): tested by `bun test src/lib/intro.test.ts`. The device side is lib/intro-sync.ts.
import { CAST, type Who } from '@/lib/mascot';

/**
 * What this device remembers for the signed-in session (`qdn.introDone`), cleared on sign-out and at each sign-in:
 * - `no`: the server said no intro (older account, party guest, a server without it): never asked again.
 * - `done` / `skipped`: finished on this device, the server not told yet (offline): never shown again, told at the next start.
 * - nothing: unknown, the server is asked.
 */
export const INTRO_KEY = 'qdn.introDone';
export type IntroMark = 'no' | 'done' | 'skipped';

export const readMark = (v: string | null | undefined): IntroMark | null => (v === 'no' || v === 'done' || v === 'skipped' ? v : null);

/** At start: `ask` the server, `resend` the finish it never got (no intro either way), or nothing to do. */
export function introPlan(mark: IntroMark | null): 'ask' | 'resend' | 'none' {
  if (mark === 'done' || mark === 'skipped') return 'resend';
  return mark === 'no' ? 'none' : 'ask';
}

/**
 * The server's answer. `features` null: they couldn't be read (offline). `show` null: not asked or no answer.
 * `mark`: what to remember (null: nothing, ask again next time: unknown, or the intro is still to do).
 */
export function introAnswer(features: readonly string[] | null | undefined, show: boolean | null | undefined): { show: boolean; mark: IntroMark | null } {
  if (!features) return { show: false, mark: null };
  // A server without the intro: none, for good (an account made before it gets backfilled as done anyway).
  if (!features.includes('intro')) return { show: false, mark: 'no' };
  if (show == null) return { show: false, mark: null };
  return show ? { show: true, mark: null } : { show: false, mark: 'no' };
}

/** `?intro=1` on the web: the intro again, for screenshots and QA (signed in; finishing it tells the server nothing). */
export const introPreview = (search: string) => new URLSearchParams(search).get('intro') === '1';

// ---------- the demo round ----------

/** Who each Peton votes for: the band always picks Bulle (the shy one, always the one who gets picked); Bulle blames Pavé. */
export const DEMO_VOTES: Record<Who, Who> = { zeste: 'bulle', pave: 'bulle', paillette: 'bulle', roupi: 'bulle', bulle: 'pave' };
/** The order their votes come in after mine: Roupi, asleep, last. */
export const DEMO_ORDER: Who[] = ['zeste', 'paillette', 'pave', 'bulle', 'roupi'];

export type Voter = Who | 'me';
export type DemoRow = { who: Who; voters: Voter[]; pct: number; win: boolean };

/** The result once I voted for `pick`: one row per Peton with votes, most first (the cast's order on a tie). */
export function demoTally(pick: Who): DemoRow[] {
  const ballots: [Voter, Who][] = [['me', pick], ...DEMO_ORDER.map((w): [Voter, Who] => [w, DEMO_VOTES[w]])];
  const total = ballots.length;
  const rows = CAST.map((who) => ({ who, voters: ballots.filter(([, to]) => to === who).map(([from]) => from) }))
    .filter((r) => r.voters.length > 0)
    .sort((a, b) => b.voters.length - a.voters.length || CAST.indexOf(a.who) - CAST.indexOf(b.who));
  const top = rows[0]?.voters.length ?? 0;
  return rows.map((r) => ({ ...r, pct: Math.round((r.voters.length / total) * 100), win: r.voters.length === top }));
}
