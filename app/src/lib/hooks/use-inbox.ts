import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { inboxKeys, markInboxRead } from '@/lib/api/inbox';

export function useMarkInboxRead(unread: number) {
  const qc = useQueryClient();
  useEffect(() => {
    if (unread > 0) markInboxRead().then(() => qc.invalidateQueries({ queryKey: inboxKeys.all }));
  }, [unread, qc]);
}
