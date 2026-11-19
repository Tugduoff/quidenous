import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { useFeatures } from '@/lib/api/features';
import { type ActiveSkin, pickSkin, type SeasonInfo } from '@/lib/utils/season';

export const seasonKeys = {
  all: ['season'] as const,
};

const SEASON_STALE_MS = 6 * 3_600_000;

const skinToday = (info: SeasonInfo) => pickSkin(info, new Date());

export function useSeasonToday(wanted: boolean): ActiveSkin | null {
  const { seasons } = useFeatures();
  const enabled = seasons && wanted;
  const { data } = useQuery({
    queryKey: seasonKeys.all,
    queryFn: () => api<SeasonInfo>('/season'),
    select: skinToday,
    enabled,
    staleTime: SEASON_STALE_MS,
  });
  return enabled ? (data ?? null) : null;
}
