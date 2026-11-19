import { ACCENTS, ROOM_THEMES, type Accent } from '@/lib/tokens';

export type CuratedId = keyof typeof ROOM_THEMES;
export type RoomThemeId = Accent | CuratedId;
export type CuratedTheme = (typeof ROOM_THEMES)[CuratedId];

export const CURATED_IDS = Object.keys(ROOM_THEMES) as CuratedId[];
export const ACCENT_IDS = Object.keys(ACCENTS) as Accent[];

export const isCurated = (id: string): id is CuratedId => Object.hasOwn(ROOM_THEMES, id);
const isAccent = (id: string): id is Accent => Object.hasOwn(ACCENTS, id);

export function cleanTheme(id: string | null | undefined): RoomThemeId | null {
  return id && (isCurated(id) || isAccent(id)) ? id : null;
}

export function curated(id: RoomThemeId | null | undefined): CuratedTheme | null {
  return id && isCurated(id) ? ROOM_THEMES[id] : null;
}
