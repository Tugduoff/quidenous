import { router } from 'expo-router';
import type { InboxItem } from '@/lib/api/inbox';
import { useSwitchRoom, useSwitchRoomHere } from '@/lib/hooks/use-switch-room';

export function useOpenInboxItem(room: number | null | undefined) {
  const switchRoom = useSwitchRoom();
  const switchHere = useSwitchRoomHere();
  return (item: InboxItem) => {
    // Not about this room: a friend request (Friends) or an invitation into another room (my rooms).
    if (item.kind === 'friend_request') return router.push('/friends');
    if (item.kind === 'invite') return router.push('/rooms');
    // About me, whatever the room: my collection, my year.
    if (item.kind === 'badge') return router.push('/badges');
    if (item.kind === 'wrapped') return router.push('/wrapped');
    // Added to a room, or now its owner: go to it (switching rooms lands on its home screen).
    if (item.kind === 'added' || item.kind === 'owner') {
      if (item.room_id != null && item.room_id !== room) switchRoom(item.room_id);
      else router.replace('/');
      return;
    }
    // Someone asks me (the owner) for questions: the Questions tab, in that room.
    if (item.kind === 'ask') {
      if (item.room_id != null && item.room_id !== room)
        switchHere(item.room_id)
          .catch(() => false)
          .then((ok) => router.replace(ok ? '/questions' : '/'));
      else router.replace('/questions');
      return;
    }
    // A message or mention under a result of another room: open it in that room (one I left: home instead).
    if ((item.kind === 'mention' || item.kind === 'message') && item.round_id != null && item.room_id != null && item.room_id !== room) {
      const id = item.round_id;
      switchHere(item.room_id)
        .catch(() => false)
        .then((ok) => (ok ? router.push(`/round/${id}`) : router.replace('/')));
      return;
    }
    if (item.kind === 'nudge' || item.kind === 'question' || item.round_id == null) router.replace('/');
    else router.push(`/round/${item.round_id}`);
  };
}
