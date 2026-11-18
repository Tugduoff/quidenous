import { getLang, t } from '@/lib/i18n';
import type { Group, Member } from '@/lib/api/group';

/** What the server writes in place of a deleted account's name (French, before `deleted` existed). */
const DELETED_NAME = 'Compte supprimé';

type Named = { name: string; deleted?: boolean };

/** A deleted account: flagged by the server, or the French placeholder name of an older one. */
const isDeleted = (m: Named) => !!m.deleted || m.name === DELETED_NAME;

/** The same person with the name to show (the same object when nothing changes). */
function localizeMember<T extends Named>(m: T): T {
  return isDeleted(m) ? { ...m, name: t('common.deletedAccount') } : m;
}

/** The people of a list, names to show. */
export const localizeMembers = <T extends Named>(list: T[]) => (list.some(isDeleted) ? list.map(localizeMember) : list);

// Per server answer and language, so every screen gets the same object back (no effect re-runs).
const groups = new WeakMap<Group, { lang: string; group: Group }>();

/** A room as the app shows it: deleted accounts named in my language. */
export function localizeGroup(g: Group): Group {
  const lang = getLang();
  const hit = groups.get(g);
  if (hit && hit.lang === lang) return hit.group;
  const everyone: Member[] = [...g.members, ...(g.former ?? [])];
  const group = everyone.some(isDeleted) ? { ...g, members: localizeMembers(g.members), former: g.former && localizeMembers(g.former) } : g;
  groups.set(g, { lang, group });
  return group;
}

/** "Léa Martin" → "Léa": chips, signatures and narrow columns stay short. */
export const firstName = (name: string) => name.trim().split(/\s+/)[0] || name;

export const nameOf = (byId: Map<number, Member>, id: number | null) => (id == null ? undefined : byId.get(id))?.name ?? '?';
