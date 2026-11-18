import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { acceptTerms, authKeys, useAccount } from '@/lib/api/auth';

let ticked = false;

export const tickTerms = (on: boolean) => {
  ticked = on;
};

export function useTermsGate() {
  const qc = useQueryClient();
  const { data: account } = useAccount();
  const accept = useMutation({
    mutationFn: () => acceptTerms(),
    onSuccess: () => {
      ticked = false;
      qc.invalidateQueries({ queryKey: authKeys.account });
    },
  });
  const missing = account?.terms === false;
  const silent = missing && ticked;
  const { mutate } = accept;
  const accepted = account?.terms;
  useEffect(() => {
    if (silent) mutate();
    else if (accepted) ticked = false;
  }, [silent, mutate, accepted]);
  return { accept, asking: missing && !silent };
}
