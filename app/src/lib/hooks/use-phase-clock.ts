import { useQueryClient } from '@tanstack/react-query';
import { type Party, partyKeys, type PartyRound } from '@/lib/api/party';
import { useCountdown } from '@/lib/hooks/use-countdown';
import { useRefetchAtZero } from '@/lib/hooks/use-refetch-at-zero';

/** The countdown of the current phase, refetching once it runs out in case the server's event got lost. */
export function usePhaseClock(party: Party, round: PartyRound) {
  const qc = useQueryClient();
  const ms = useCountdown(round.deadline_at, party.clock);
  useRefetchAtZero(ms, () => qc.invalidateQueries({ queryKey: partyKeys.party(party.id) }));
  return ms;
}
