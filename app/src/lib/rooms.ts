import { useQueryClient } from '@tanstack/react-query';
import { router, useGlobalSearchParams } from 'expo-router';
import { useEffect, useRef } from 'react';

import { api, setApiRoom, type RoomSummary } from './api';
import { useSession } from './session';

/** After leaving or deleting the current room: on to another room I'm in, else the room list. */
export function useRoomGone() {
  const qc = useQueryClient();
  const { setRoom } = useSession();
  return async () => {
    await setRoom(null);
    qc.clear();
    const rooms = await api<{ id: number }[]>('/rooms').catch(() => []);
    // From a page deep in the stack (the room's settings): back to the bottom first.
    if (router.canDismiss()) router.dismissAll();
    if (rooms.length) {
      await setRoom(rooms[0].id);
      router.replace('/');
    } else router.replace('/rooms');
  };
}

/** Switch the whole app to another room: every cached screen belongs to the old one. */
export function useSwitchRoom() {
  const qc = useQueryClient();
  const { setRoom } = useSession();
  return async (id: number) => {
    await setRoom(id);
    qc.clear();
    router.replace('/');
  };
}

/**
 * Switch rooms but stay on the screen about to open (a notification for a question of another room).
 * Only to a room I'm still in. The room header changes before the cache is dropped, so every refetch asks the new room.
 */
export function useSwitchRoomHere() {
  const qc = useQueryClient();
  const { setRoom } = useSession();
  return async (id: number) => {
    const rooms = await api<RoomSummary[]>('/rooms').catch(() => null);
    if (rooms && !rooms.some((r) => r.id === id)) return false;
    setApiRoom(id);
    qc.clear();
    await setRoom(id);
    return true;
  };
}

/** `?room=<id>` on any screen (a notification tap): switch to that room right there, then drop the param. */
export function useRoomParam(active: boolean) {
  const { room: param } = useGlobalSearchParams<{ room?: string }>();
  const { room } = useSession();
  const switchHere = useSwitchRoomHere();
  const handled = useRef<string | null>(null);
  useEffect(() => {
    if (!param) handled.current = null;
    if (!active || !param || handled.current === param) return;
    handled.current = param;
    router.setParams({ room: undefined });
    const id = Number(param);
    if (!Number.isInteger(id) || id <= 0 || id === room) return;
    // A room I'm no longer in: its screens would fail, home instead.
    switchHere(id)
      .catch(() => false)
      .then((ok) => ok || router.replace('/'));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, param, room]);
}
