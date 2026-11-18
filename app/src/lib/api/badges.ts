import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import type { Who } from '@/lib/utils/mascot';
import { useGroup, groupKeys } from '@/lib/api/group';
import { useFeatures } from '@/lib/api/features';

export type Tier = 0 | 1 | 2 | 3;
export type Badge = {
  key: string;
  icon: string;
  mascot: Who;
  name: string;
  hint: string;
  tier: Tier;
  tiers: number[];
  progress: number;
  earned_at: string | null;
};
export type TitleChip = { key: string; name: string; icon: string; tier: 1 | 2 | 3 };
export type Badges = { title: string | null; earned: number; total: number; badges: Badge[] };
type Titles = { members: Record<string, TitleChip> };
export type MemberBadges = { title: TitleChip | null; badges: Pick<Badge, 'key' | 'name' | 'icon' | 'mascot' | 'tier'>[] };

export const badgeKeys = {
  mine: ['badges'] as const,
  titles: [...groupKeys.all, 'titles'] as const,
  member: (id: number) => [...groupKeys.all, 'badges', id] as const,
};

export function useBadges() {
  const { badges } = useFeatures();
  return useQuery({ queryKey: badgeKeys.mine, queryFn: () => api<Badges>('/badges'), enabled: badges, staleTime: 60_000 });
}

function useTitlesOn() {
  const { badges } = useFeatures();
  const { data: group } = useGroup();
  return badges && !!group && group.kind !== 'party';
}

const titlesQuery = { queryKey: badgeKeys.titles, queryFn: () => api<Titles>('/titles'), staleTime: 60_000 };

export function useMemberTitle(id: number | null | undefined): TitleChip | null {
  const on = useTitlesOn();
  const { data } = useQuery({
    ...titlesQuery,
    enabled: on && id != null,
    select: (d: Titles) => (id == null ? null : (d.members?.[String(id)] ?? null)),
  });
  return on ? (data ?? null) : null;
}

export function useMemberBadges(id: number | null, enabled = true) {
  const on = useTitlesOn();
  return useQuery({
    queryKey: badgeKeys.member(id ?? 0),
    queryFn: () => api<MemberBadges>(`/members/${id}/badges`),
    enabled: on && enabled && id != null,
    staleTime: 60_000,
  });
}

export const chipOf = (b: Badge): TitleChip | null => (b.tier > 0 ? { key: b.key, name: b.name, icon: b.icon, tier: b.tier as 1 | 2 | 3 } : null);

export function useMyTitle(): TitleChip | null {
  const { data } = useBadges();
  const b = data?.title ? data.badges.find((x) => x.key === data.title) : undefined;
  return b ? chipOf(b) : null;
}

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
