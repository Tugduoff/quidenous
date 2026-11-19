import { createContext, type ReactNode, useContext } from 'react';
import { useSeasonToday } from '@/lib/api/season';
import { presetSeason } from '@/lib/device/launch-params';
import { useSeasonalPref } from '@/lib/device/prefs';
import { type ActiveSkin, overrideSkin, type SeasonId } from '@/lib/utils/season';

const FORCED = presetSeason();
const FORCED_SKIN = FORCED ? overrideSkin(FORCED) : null;

const SeasonContext = createContext<ActiveSkin | null>(null);

export function SeasonScope({ children }: { children: ReactNode }) {
  const wanted = useSeasonalPref();
  const today = useSeasonToday(wanted && !FORCED);
  return <SeasonContext.Provider value={FORCED ? FORCED_SKIN : today}>{children}</SeasonContext.Provider>;
}

export const useSeasonSkin = (): ActiveSkin | null => useContext(SeasonContext);

export const useSeason = (): SeasonId | null => useSeasonSkin()?.id ?? null;
