import { type ReactNode, useContext } from 'react';

import { useRoomThemeId } from '@/lib/hooks/use-room-theme';
import { RoomLookContext } from '@/lib/theme';

export function RoomThemeScope({ room, children }: { room: boolean; children: ReactNode }) {
  if (!room) return children;
  return <InRoom>{children}</InRoom>;
}

function InRoom({ children }: { children: ReactNode }) {
  const id = useRoomThemeId();
  return <RoomLookContext.Provider value={id}>{children}</RoomLookContext.Provider>;
}

export function OutsideRoom({ when, children }: { when: boolean; children: ReactNode }) {
  const look = useContext(RoomLookContext);
  return <RoomLookContext.Provider value={when ? null : look}>{children}</RoomLookContext.Provider>;
}
