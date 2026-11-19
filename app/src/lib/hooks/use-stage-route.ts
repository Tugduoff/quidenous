import { router, useGlobalSearchParams, usePathname } from 'expo-router';
import { useEffect, useRef } from 'react';
import { presetJoinCode } from '@/lib/device/launch-params';
import { pendingJoinCode, pendingPlayToken, setPendingCode } from '@/lib/auth-flow';

export type Stage = 'out' | 'verify' | 'intro' | 'room' | 'in';
const STAGE_HOME: Record<Stage, string> = { out: '/join', verify: '/auth/verify', intro: '/intro', room: '/setup', in: '/' };
const ROOMLESS = ['/setup', '/profile', '/friends', '/security', '/notifications', '/badges', '/wrapped'];
const SIGNED_OUT = ['/join', '/auth/email', '/auth/password', '/auth/two-factor', '/auth/forgot', '/auth/reset', '/auth/mfa-reset'];

let presetUsed = false;

export function useStageRoute(stage: Stage, mounted: boolean) {
  const pathname = usePathname();
  const { code, error, share } = useGlobalSearchParams<{ code?: string; error?: string; share?: string }>();
  const wasOut = useRef(false);
  const invited = useRef(false);
  useEffect(() => {
    if (!mounted) return;
    if (stage === 'out' || stage === 'verify') wasOut.current = true;
    if (pathname === '/oauth' && (code || error)) return;
    if ((stage === 'in' || stage === 'room') && wasOut.current) {
      wasOut.current = false;
      if (pendingPlayToken() && pathname !== '/play') return void router.replace('/play');
    }
    const open = pathname.startsWith('/party/join/') || pathname.startsWith('/party/screen/');
    const play = pathname === '/play' && stage !== 'verify' && stage !== 'intro' && !!(share || pendingPlayToken());
    if (stage !== 'in') presetUsed = true;
    if (stage === 'in' && pathname === '/join' && (code || (!presetUsed && presetJoinCode()))) {
      setPendingCode(code ?? presetJoinCode());
      invited.current = true;
      presetUsed = true;
    }
    if (stage === 'in' && pathname === '/' && !presetUsed && presetJoinCode()) {
      presetUsed = true;
      setPendingCode(presetJoinCode());
      invited.current = true;
    }
    if (stage === 'in' && pathname === '/' && invited.current && pendingJoinCode()) {
      invited.current = false;
      return void router.push('/rooms');
    }
    const ok =
      open || play
        ? true
        : stage === 'out'
          ? SIGNED_OUT.includes(pathname)
          : stage === 'verify'
            ? pathname === '/auth/verify'
            : stage === 'intro'
              ? pathname === '/intro'
              : stage === 'room'
                ? ROOMLESS.includes(pathname)
                : pathname !== '/oauth' && pathname !== '/setup' && pathname !== '/join' && pathname !== '/intro' && !pathname.startsWith('/auth');
    if (!ok) router.replace(STAGE_HOME[stage] as never);
  }, [stage, pathname, mounted, code, error, share]);
}
