import { getLang, t } from '@/lib/i18n';
import type { Group, Member } from '@/lib/api/group';

const DELETED_NAME = 'Compte supprimé';

type Named = { name: string; deleted?: boolean };

const isDeleted = (m: Named) => !!m.deleted || m.name === DELETED_NAME;

function localizeMember<T extends Named>(m: T): T {
  return isDeleted(m) ? { ...m, name: t('common.deletedAccount') } : m;
}

export const localizeMembers = <T extends Named>(list: T[]) => (list.some(isDeleted) ? list.map(localizeMember) : list);

const groups = new WeakMap<Group, { lang: string; group: Group }>();

export function localizeGroup(g: Group): Group {
  const lang = getLang();
  const hit = groups.get(g);
  if (hit && hit.lang === lang) return hit.group;
  const everyone: Member[] = [...g.members, ...(g.former ?? [])];
  const group = everyone.some(isDeleted) ? { ...g, members: localizeMembers(g.members), former: g.former && localizeMembers(g.former) } : g;
  groups.set(g, { lang, group });
  return group;
}

export const firstName = (name: string) => name.trim().split(/\s+/)[0] || name;

export const nameOf = (byId: Map<number, Member>, id: number | null) => (id == null ? undefined : byId.get(id))?.name ?? '?';
