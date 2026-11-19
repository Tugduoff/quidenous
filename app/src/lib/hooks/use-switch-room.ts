import { useQueryClient } from '@tanstack/react-query';
import { router, useGlobalSearchParams } from 'expo-router';
import { useEffect, useRef, useEffectEvent } from 'react';
import { setApiRoom } from '@/lib/api/client';
import { clearRoomData } from '@/lib/api/query-client';
import { useSession } from '@/lib/session';
import { listRooms, warmRoom } from '@/lib/api/rooms';

export function useRoomGone() {
  const qc = useQueryClient();
  const { setRoom, noRoomLeft } = useSession();
  return async () => {
    setApiRoom(null);
    const rooms = await listRooms().catch(() => null);
    if (router.canDismiss()) router.dismissAll();
    if (rooms?.length) {
      await setRoom(rooms[0].id);
      clearRoomData(qc);
      router.replace('/');
    } else if (rooms) {
      await noRoomLeft();
      clearRoomData(qc);
    } else {
      await setRoom(null);
      clearRoomData(qc);
      router.replace('/rooms');
    }
  };
}

export function useSwitchRoom() {
  const qc = useQueryClient();
  const { setRoom } = useSession();
  return async (id: number, kind?: 'room' | 'party', { warm = false } = {}) => {
    const warming = warm && kind !== 'party' ? warmRoom(id) : null;
    await setRoom(id);
    const fresh = (await warming) ?? [];
    clearRoomData(qc);
    for (const [key, data] of fresh) qc.setQueryData(key, data);
    router.replace(kind === 'party' ? `/party/${id}` : '/');
  };
}

export function useSwitchRoomHere() {
  const qc = useQueryClient();
  const { setRoom } = useSession();
  return async (id: number) => {
    const rooms = await listRooms().catch(() => null);
    if (rooms && !rooms.some((r) => r.id === id)) return false;
    setApiRoom(id);
    clearRoomData(qc);
    await setRoom(id);
    return true;
  };
}

export function useRoomParam(active: boolean) {
  const { room: param } = useGlobalSearchParams<{ room?: string }>();
  const { room } = useSession();
  const switchHere = useSwitchRoomHere();
  const switchTo = useEffectEvent((id: number) => switchHere(id));
  const handled = useRef<string | null>(null);
  useEffect(() => {
    if (!param) handled.current = null;
    if (!active || !param || handled.current === param) return;
    handled.current = param;
    router.setParams({ room: undefined });
    const id = Number(param);
    if (!Number.isInteger(id) || id <= 0 || id === room) return;
    switchTo(id)
      .catch(() => false)
      .then((ok) => ok || router.replace('/'));
  }, [active, param, room]);
}
