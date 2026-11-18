import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { type Round, roundKeys } from '@/lib/api/rounds';
import { useGroup } from '@/lib/api/group';
import { statsKeys } from '@/lib/api/stats';
import { useFeatures } from '@/lib/api/features';

/** Le Tripot (docs/TRIPOT.md): bets on who wins a question, with chips worth nothing (no real money, ever). */
type MyBet = { target_id: number; amount: number; payout: number | null };
type TripotWinner = { member_id: number; stake: number; won: number };
/** On a round: open, only the pot, the number of bettors and my own bet; closed, who won how much (or `refunded`). */
export type Tripot = {
  pot: number;
  bettors: number;
  my_bet: MyBet | null;
  balance: number;
  can_bet: boolean;
  winners: TripotWinner[] | null;
  refunded: boolean;
};
type ChipCount = { member_id: number; balance: number };
/** `GET /tripot`: this month's chips (most first), last month's top 3. */
export type TripotBoard = {
  month: string;
  last_month: string;
  start: number;
  daily: number;
  cap: number;
  balances: ChipCount[];
  podium: ChipCount[];
};

/** The quick stakes in the bet sheet (plus "tapis": everything I have). */
export const CHIPS = [10, 25, 50] as const;

/** Under the stats key: a closed question (live event) refreshes it with them. */
const tripotKeys = {
  board: [...statsKeys.all, 'tripot'] as const,
};

/** Tripot is on here: the server has it, the room is not a party, and its owner left it on. */
function useTripotOn() {
  const { tripot } = useFeatures();
  const { data: group } = useGroup();
  return tripot && !!group && group.kind !== 'party' && group.settings.tripot === true;
}

export function useTripotBoard() {
  const on = useTripotOn();
  return useQuery({ queryKey: tripotKeys.board, queryFn: () => api<TripotBoard>('/tripot'), enabled: on, staleTime: 30_000 });
}

/** One bet on the open question; the round shows it right away. */
export function useBet(round: Round) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { target: number; amount: number }) => api<Tripot | null>(`/rounds/${round.id}/bet`, { method: 'POST', json: body }),
    onSuccess: (tripot) => {
      if (tripot) qc.setQueryData<Round | null>(roundKeys.current, (r) => (r && r.id === round.id ? { ...r, tripot } : r));
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: roundKeys.current });
      qc.invalidateQueries({ queryKey: tripotKeys.board });
    },
  });
}
