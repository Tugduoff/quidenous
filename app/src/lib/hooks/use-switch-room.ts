import { useQueryClient } from '@tanstack/react-query';
import { router, useGlobalSearchParams } from 'expo-router';
import { useEffect, useRef } from 'react';
import { setApiRoom } from '@/lib/api/client';
import { clearRoomData } from '@/lib/api/query-client';
import { useSession } from '@/lib/session';
import { listRooms, warmRoom } from '@/lib/api/rooms';

/**
 * After leaving or deleting the current room: on to another room I'm in. None left: join or create one (/setup).
 * The list unknown (offline): the room list, which refetches.
 */
export function useRoomGone() {
  const qc = useQueryClient();
  const { setRoom, noRoomLeft } = useSession();
  return async () => {
    // Requests stop naming the room at once; the room on screen only changes once the next one is known, so the
    // live socket and `/me` aren't asked again for no room right before the "join or create" screen.
    setApiRoom(null);
    const rooms = await listRooms().catch(() => null);
    // From a page deep in the stack (the room's settings): back to the bottom first.
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

/**
 * Switch the whole app to another room: every cached screen belongs to the old one. A party opens on its own screen.
 * `warm`: the vote tab opens already filled in (« Aujourd'hui », where the tap names what to do there).
 */
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

/**
 * Switch rooms but stay on the screen about to open (a notification for a question of another room).
 * Only to a room I'm still in. The room header changes before the cache is dropped, so every refetch asks the new room.
 */
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
