import { useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';

import { useSession } from './session';

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
