import { useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { useCallback } from 'react';
import { todayKeys } from '@/lib/api/today';
import { useLatest } from '@/lib/hooks/use-latest';
import { useSwitchRoom } from '@/lib/hooks/use-switch-room';
import { useSession } from '@/lib/session';

export function useOpenRoom() {
  const { room } = useSession();
  const switchRoom = useLatest(useSwitchRoom());
  const qc = useQueryClient();
  return useCallback(
    (id: number) => {
      if (id === room) return router.navigate('/');
      switchRoom.current(id, 'room', { warm: true }).catch(() => qc.invalidateQueries({ queryKey: todayKeys.all }));
    },
    [room, switchRoom, qc],
  );
}
