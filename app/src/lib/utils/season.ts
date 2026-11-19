const KNOWN = {
  winter: { from: '12-01', until: '01-06' },
} as const;
export type SeasonId = keyof typeof KNOWN;

type Skin = { id: string; from: string; until: string };
export type SeasonInfo = { active: string | null; skins: Skin[] };
export type ActiveSkin = { id: SeasonId; from: string; until: string };
export type SeasonOverride = SeasonId | 'off';

const isKnown = (id: unknown): id is SeasonId => typeof id === 'string' && Object.prototype.hasOwnProperty.call(KNOWN, id);

const knownSkin = (id: SeasonId): ActiveSkin => ({ id, ...KNOWN[id] });

const MONTH_DAY = /^(\d\d)-(\d\d)$/;

function monthDay(text: unknown): number | null {
  const match = typeof text === 'string' ? MONTH_DAY.exec(text) : null;
  if (!match) return null;
  const [month, day] = [+match[1], +match[2]];
  return month >= 1 && month <= 12 && day >= 1 && day <= 31 ? month * 100 + day : null;
}

const localMonthDay = (date: Date) => (date.getMonth() + 1) * 100 + date.getDate();

export function inRange(from: string, until: string, date: Date): boolean {
  const [start, end] = [monthDay(from), monthDay(until)];
  if (start == null || end == null) return false;
  const today = localMonthDay(date);
  const wrapsYearEnd = start > end;
  return wrapsYearEnd ? today >= start || today <= end : today >= start && today <= end;
}

export function activeSkin(skins: unknown, date: Date): ActiveSkin | null {
  if (!Array.isArray(skins)) return null;
  for (const skin of skins as Partial<Skin>[]) {
    if (skin && isKnown(skin.id) && typeof skin.from === 'string' && typeof skin.until === 'string' && inRange(skin.from, skin.until, date))
      return { id: skin.id, from: skin.from, until: skin.until };
  }
  return null;
}

export function pickSkin(info: Partial<SeasonInfo> | null | undefined, date: Date): ActiveSkin | null {
  if (!info) return null;
  if (Array.isArray(info.skins)) return activeSkin(info.skins, date);
  return isKnown(info.active) ? knownSkin(info.active) : null;
}

export function parseOverride(search: string): SeasonOverride | null {
  const value = new URLSearchParams(search).get('season');
  return value === 'off' || isKnown(value) ? value : null;
}

export const overrideSkin = (override: SeasonOverride): ActiveSkin | null => (override === 'off' ? null : knownSkin(override));

const NEW_YEAR = 101;

export function seasonTag(skin: Pick<ActiveSkin, 'id' | 'from'>, date: Date): string {
  const from = monthDay(skin.from) ?? NEW_YEAR;
  const startedLastYear = localMonthDay(date) < from;
  return `${skin.id}-${date.getFullYear() - (startedLastYear ? 1 : 0)}`;
}
