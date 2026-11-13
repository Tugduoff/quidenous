import { useQuery, useQueryClient } from '@tanstack/react-query';

import { api, type Member } from './api';
import { keys } from './queries';
import { useSession } from './session';

export type ReportKind = 'message' | 'member' | 'question';
export type ReportReason = 'offensive' | 'harassment' | 'sexual' | 'spam' | 'other';

export const REASONS: { value: ReportReason; label: string }[] = [
  { value: 'offensive', label: 'Offensant' },
  { value: 'harassment', label: 'Harcèlement' },
  { value: 'sexual', label: 'Contenu sexuel' },
  { value: 'spam', label: 'Spam' },
  { value: 'other', label: 'Autre' },
];

/** Someone I blocked (`GET /api/blocks`). */
export type Blocked = { user_id: number; name: string; color: string; photo: string | null; avatar?: string | null };

export const report = (kind: ReportKind, id: number, reason: ReportReason, details?: string) =>
  api<{ ok: boolean }>('/reports', { method: 'POST', json: { kind, id, reason, ...(details && { details }) } });

/** `photo` may be the stored file name or already a `/photos/…` path. */
export const blockedAsMember = (b: Blocked): Member => ({
  id: b.user_id,
  name: b.name,
  color: b.color,
  photo_url: b.photo ? (b.photo.startsWith('/') ? b.photo : `/photos/${b.photo}`) : null,
  avatar: b.avatar,
});

const NONE = new Set<number>();

/** People I blocked, shared by every screen (an older server without blocks: nobody). */
export function useBlocks() {
  const { token } = useSession();
  const { data } = useQuery({ queryKey: keys.blocks, queryFn: () => api<Blocked[]>('/blocks'), enabled: !!token, retry: false });
  const list = data ?? [];
  return { list, ids: data ? new Set(list.map((b) => b.user_id)) : NONE };
}

/** Block / unblock; the list changes at once, then the screens they touch reload. */
export function useBlockActions() {
  const qc = useQueryClient();
  const refresh = () => {
    qc.invalidateQueries({ queryKey: keys.blocks });
    qc.invalidateQueries({ queryKey: ['messages'] });
    qc.invalidateQueries({ queryKey: keys.inbox });
    qc.invalidateQueries({ queryKey: keys.friends });
    qc.invalidateQueries({ queryKey: keys.rooms });
  };
  return {
    block: async (who: Blocked) => {
      await api(`/blocks/${who.user_id}`, { method: 'PUT' });
      qc.setQueryData<Blocked[]>(keys.blocks, (old) => [...(old ?? []).filter((b) => b.user_id !== who.user_id), who]);
      refresh();
    },
    unblock: async (userId: number) => {
      await api(`/blocks/${userId}`, { method: 'DELETE' });
      qc.setQueryData<Blocked[]>(keys.blocks, (old) => old?.filter((b) => b.user_id !== userId));
      refresh();
    },
  };
}
