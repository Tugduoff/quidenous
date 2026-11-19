import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useEffectEvent } from 'react';
import { connectLive } from '@/lib/api/live';
import { connectScreen } from '@/lib/api/party';

export function useLive(token: string | null, room: number | null) {
  const qc = useQueryClient();
  useEffect(() => {
    if (!token) return;
    return connectLive(qc, token);
  }, [token, room, qc]);
}

export function useScreenLive(code: string, onEvent: () => void) {
  const qc = useQueryClient();
  const changed = useEffectEvent(() => onEvent());
  useEffect(() => {
    if (!code) return;
    return connectScreen(qc, code, () => changed());
  }, [code, qc]);
}
