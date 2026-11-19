import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { type Round, roundKeys } from '@/lib/api/rounds';
import { useGroup } from '@/lib/api/group';
import { statsKeys } from '@/lib/api/stats';
import { useFeatures } from '@/lib/api/features';

type MyBet = { target_id: number; amount: number; payout: number | null };
type TripotWinner = { member_id: number; stake: number; won: number };
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
export type TripotBoard = {
  month: string;
  last_month: string;
  start: number;
  daily: number;
  cap: number;
  balances: ChipCount[];
  podium: ChipCount[];
};

export const CHIPS = [10, 25, 50] as const;

const tripotKeys = {
  board: [...statsKeys.all, 'tripot'] as const,
};

function useTripotOn() {
  const { tripot } = useFeatures();
  const { data: group } = useGroup();
  return tripot && !!group && group.kind !== 'party' && group.settings.tripot === true;
}

export function useTripotBoard() {
  const on = useTripotOn();
  return useQuery({ queryKey: tripotKeys.board, queryFn: () => api<TripotBoard>('/tripot'), enabled: on, staleTime: 30_000 });
}

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
