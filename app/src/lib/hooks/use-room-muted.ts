import { useFeatures } from '@/lib/api/features';
import { useRooms } from '@/lib/api/rooms';

export function useRoomMuted(id: number | undefined) {
  const features = useFeatures();
  const { data: rooms } = useRooms();
  if (!features.notificationSettings || id == null) return undefined;
  return rooms?.find((r) => r.id === id)?.muted;
}
