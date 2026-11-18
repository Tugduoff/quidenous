import { useQueryClient } from '@tanstack/react-query';
import { removeMember } from '@/lib/api/group';
import { partyError, partyKeys } from '@/lib/api/party';
import { askConfirm } from '@/lib/confirm';
import { useT } from '@/lib/i18n';
import { showToast } from '@/lib/toast';

/** Owner: takes a player out of the party, after asking (their name stays taken: no fresh seat by rescanning). */
export function useRemovePlayer(id: number) {
  const tr = useT();
  const qc = useQueryClient();
  return async (p: { id: number; name: string }) => {
    if (!(await askConfirm(tr('party.remove.ask', { name: p.name }), undefined, tr('party.remove.action')))) return;
    try {
      await removeMember(id, p.id);
      await qc.invalidateQueries({ queryKey: partyKeys.party(id) });
    } catch (e) {
      showToast(partyError(e));
    }
  };
}
