import { CAST, type Who } from '@/lib/utils/mascot';

export const INTRO_KEY = 'qdn.introDone';
export type IntroMark = 'no' | 'done' | 'skipped';

export const readMark = (v: string | null | undefined): IntroMark | null => (v === 'no' || v === 'done' || v === 'skipped' ? v : null);

export function introPlan(mark: IntroMark | null): 'ask' | 'resend' | 'none' {
  if (mark === 'done' || mark === 'skipped') return 'resend';
  return mark === 'no' ? 'none' : 'ask';
}

export function introAnswer(features: readonly string[] | null | undefined, show: boolean | null | undefined): { show: boolean; mark: IntroMark | null } {
  if (!features) return { show: false, mark: null };
  if (!features.includes('intro')) return { show: false, mark: 'no' };
  if (show == null) return { show: false, mark: null };
  return show ? { show: true, mark: null } : { show: false, mark: 'no' };
}

export const introPreview = (search: string) => new URLSearchParams(search).get('intro') === '1';

const DEMO_VOTES: Record<Who, Who> = { zeste: 'bulle', pave: 'bulle', paillette: 'bulle', roupi: 'bulle', bulle: 'pave' };
export const DEMO_ORDER: Who[] = ['zeste', 'paillette', 'pave', 'bulle', 'roupi'];

type Voter = Who | 'me';
export type DemoRow = { who: Who; voters: Voter[]; pct: number; win: boolean };

export function demoTally(pick: Who): DemoRow[] {
  const ballots: [Voter, Who][] = [['me', pick], ...DEMO_ORDER.map((w): [Voter, Who] => [w, DEMO_VOTES[w]])];
  const total = ballots.length;
  const rows = CAST.map((who) => ({ who, voters: ballots.filter(([, to]) => to === who).map(([from]) => from) }))
    .filter((r) => r.voters.length > 0)
    .sort((a, b) => b.voters.length - a.voters.length || CAST.indexOf(a.who) - CAST.indexOf(b.who));
  const top = rows[0]?.voters.length ?? 0;
  return rows.map((r) => ({ ...r, pct: Math.round((r.voters.length / total) * 100), win: r.voters.length === top }));
}
