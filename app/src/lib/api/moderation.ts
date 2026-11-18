import { useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { useSession } from '@/lib/session';
import type { Member } from '@/lib/api/group';
import { friendKeys } from '@/lib/api/friends';
import { roomKeys } from '@/lib/api/rooms';
import { inboxKeys } from '@/lib/api/inbox';
import { chatKeys } from '@/lib/api/chat';

export const moderationKeys = {
  blocks: ['blocks'] as const,
};

/** `answer`: an answer to a « Qui a dit ça ? » question (docs/SAID.md). */
export type ReportKind = 'message' | 'member' | 'question' | 'suggestion' | 'answer';
export type ReportReason = 'offensive' | 'harassment' | 'sexual' | 'spam' | 'other';

/** In the order shown; the label is `moderation.reasons.<reason>`. */
export const REASONS: ReportReason[] = ['offensive', 'harassment', 'sexual', 'spam', 'other'];

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
  const { data } = useQuery({ queryKey: moderationKeys.blocks, queryFn: () => api<Blocked[]>('/blocks'), enabled: !!token, retry: false });
  const list = data ?? [];
  return { list, ids: data ? new Set(list.map((b) => b.user_id)) : NONE };
}

/** Block / unblock; the list changes at once, then the screens they touch reload. */
export function useBlockActions() {
  const qc = useQueryClient();
  const refresh = () => {
    qc.invalidateQueries({ queryKey: moderationKeys.blocks });
    qc.invalidateQueries({ queryKey: chatKeys.all });
    qc.invalidateQueries({ queryKey: inboxKeys.all });
    qc.invalidateQueries({ queryKey: friendKeys.all });
    qc.invalidateQueries({ queryKey: roomKeys.all });
  };
  return {
    block: async (who: Blocked) => {
      await api(`/blocks/${who.user_id}`, { method: 'PUT' });
      qc.setQueryData<Blocked[]>(moderationKeys.blocks, (old) => [...(old ?? []).filter((b) => b.user_id !== who.user_id), who]);
      refresh();
    },
    unblock: async (userId: number) => {
      await api(`/blocks/${userId}`, { method: 'DELETE' });
      qc.setQueryData<Blocked[]>(moderationKeys.blocks, (old) => old?.filter((b) => b.user_id !== userId));
      refresh();
    },
  };
}
