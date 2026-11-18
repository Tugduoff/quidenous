import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { useFeatures } from '@/lib/api/auth';
import type { Who } from '@/lib/utils/mascot';
import { useGroup, groupKeys } from '@/lib/api/group';

/** 1 bronze, 2 silver, 3 gold (0: not earned yet). */
export type Tier = 0 | 1 | 2 | 3;
/** One badge of the catalogue, with where I stand (`GET /badges`). Name and hint come in my language. */
export type Badge = {
  key: string;
  /** Lucide name, kebab-case. */
  icon: string;
  mascot: Who;
  /** The title it gives, e.g. "Le pilier". */
  name: string;
  /** How to reach the next tier, or how it was earned once maxed. */
  hint: string;
  tier: Tier;
  /** The three thresholds. */
  tiers: number[];
  progress: number;
  earned_at: string | null;
};
/** A title shown next to someone's name. */
export type TitleChip = { key: string; name: string; icon: string; tier: 1 | 2 | 3 };
export type Badges = { title: string | null; earned: number; total: number; badges: Badge[] };
/** `GET /titles`: the titles the room's current members display, by member id. */
export type Titles = { members: Record<string, TitleChip> };
/** `GET /members/{id}/badges`: someone's earned badges (highest tier each), in my room. */
export type MemberBadges = { title: TitleChip | null; badges: Pick<Badge, 'key' | 'name' | 'icon' | 'mascot' | 'tier'>[] };

/** Mine: account-wide (cleared with the rest on a room switch, refetched on any inbox change). */
export const badgeKeys = {
  mine: ['badges'] as const,
  titles: [...groupKeys.all, 'titles'] as const,
  member: (id: number) => [...groupKeys.all, 'badges', id] as const,
};

/** My collection, from a server that has badges. */
export function useBadges() {
  const { badges } = useFeatures();
  return useQuery({ queryKey: badgeKeys.mine, queryFn: () => api<Badges>('/badges'), enabled: badges, staleTime: 60_000 });
}

/** Titles exist on this server and in this room (a party has none). */
function useTitlesOn() {
  const { badges } = useFeatures();
  const { data: group } = useGroup();
  return badges && !!group && group.kind !== 'party';
}

const titlesQuery = { queryKey: badgeKeys.titles, queryFn: () => api<Titles>('/titles'), staleTime: 60_000 };

/** Every displayed title of the room (one request, shared by every chip). */
export function useTitles() {
  const on = useTitlesOn();
  const query = useQuery({ ...titlesQuery, enabled: on });
  return on ? query.data : undefined;
}

/**
 * Someone's displayed title in this room, or null. Each chip selects its own member out of the one `/titles` query:
 * a change to someone else's title doesn't re-render it.
 */
export function useMemberTitle(id: number | null | undefined): TitleChip | null {
  const on = useTitlesOn();
  const { data } = useQuery({
    ...titlesQuery,
    enabled: on && id != null,
    select: (d: Titles) => (id == null ? null : (d.members?.[String(id)] ?? null)),
  });
  return on ? (data ?? null) : null;
}

/** Someone's earned badges (the stats profile sheet), only while `enabled`. */
export function useMemberBadges(id: number | null, enabled = true) {
  const on = useTitlesOn();
  return useQuery({
    queryKey: badgeKeys.member(id ?? 0),
    queryFn: () => api<MemberBadges>(`/members/${id}/badges`),
    enabled: on && enabled && id != null,
    staleTime: 60_000,
  });
}

/** The chip for one of my badges. */
export const chipOf = (b: Badge): TitleChip | null => (b.tier > 0 ? { key: b.key, name: b.name, icon: b.icon, tier: b.tier as 1 | 2 | 3 } : null);

/** My displayed title, from my collection. */
export function useMyTitle(): TitleChip | null {
  const { data } = useBadges();
  const b = data?.title ? data.badges.find((x) => x.key === data.title) : undefined;
  return b ? chipOf(b) : null;
}

/** Show one of my titles (`null`: none). The collection updates at once; the room's chips once the server agrees. */
export function useSetTitle() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (key: string | null) => api<{ title: string | null }>('/me/title', { method: 'PUT', json: { key } }),
    onMutate: async (key) => {
      await qc.cancelQueries({ queryKey: badgeKeys.mine });
      const before = qc.getQueryData<Badges>(badgeKeys.mine);
      qc.setQueryData<Badges>(badgeKeys.mine, (b) => b && { ...b, title: key });
      return { before };
    },
    onError: (_e, _key, ctx) => {
      if (ctx?.before) qc.setQueryData(badgeKeys.mine, ctx.before);
    },
    onSuccess: (r) => qc.setQueryData<Badges>(badgeKeys.mine, (b) => b && { ...b, title: r?.title ?? null }),
    onSettled: () => qc.invalidateQueries({ queryKey: badgeKeys.titles }),
  });
}
