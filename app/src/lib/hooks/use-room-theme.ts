import { useEffect } from 'react';

import { useFeatures } from '@/lib/api/features';
import { useRoomThemeSetting } from '@/lib/api/group';
import { rememberRoomTheme, useRememberedRoomTheme } from '@/lib/device/room-themes';
import { useSession } from '@/lib/session';
import { cleanTheme, type RoomThemeId } from '@/lib/utils/room-theme';

export function useRoomThemeId(): RoomThemeId | null {
  const { roomThemes } = useFeatures();
  const { room } = useSession();
  const { data } = useRoomThemeSetting(roomThemes && room != null);
  const fresh = data === undefined ? undefined : cleanTheme(data);
  const cached = useRememberedRoomTheme(room);
  useEffect(() => {
    if (room != null && fresh !== undefined) rememberRoomTheme(room, fresh);
  }, [room, fresh]);
  if (!roomThemes) return null;
  return fresh === undefined ? cached : fresh;
}
