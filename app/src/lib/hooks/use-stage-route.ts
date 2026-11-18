import { router, useGlobalSearchParams, usePathname } from 'expo-router';
import { useEffect, useRef } from 'react';
import { presetJoinCode } from '@/lib/device/launch-params';
import { pendingJoinCode, pendingPlayToken, setPendingCode } from '@/lib/auth-flow';

export type Stage = 'out' | 'verify' | 'intro' | 'room' | 'in';
const STAGE_HOME: Record<Stage, string> = { out: '/join', verify: '/auth/verify', intro: '/intro', room: '/setup', in: '/' };
/** What a signed-in account without a room can open besides /setup: the account itself (and deleting it). */
const ROOMLESS = ['/setup', '/profile', '/friends', '/security', '/notifications', '/badges', '/wrapped'];
const SIGNED_OUT = ['/join', '/auth/email', '/auth/password', '/auth/two-factor', '/auth/forgot', '/auth/reset', '/auth/mfa-reset'];

/**
 * Keep the screen on the current stage: when signing up, in or out changes it, go to that stage's screen.
 * Without this the router falls back to the only unguarded route (/oauth), which bounced to `/` and back forever.
 */
/** The invite code the page was opened with (read once at load) has been handled. */
let presetUsed = false;

export function useStageRoute(stage: Stage, mounted: boolean) {
  const pathname = usePathname();
  const { code, error, share } = useGlobalSearchParams<{ code?: string; error?: string; share?: string }>();
  // Seen signed out (or verifying): a shared question opened meanwhile is resumed once in.
  const wasOut = useRef(false);
  const invited = useRef(false);
  useEffect(() => {
    if (!mounted) return;
    if (stage === 'out' || stage === 'verify') wasOut.current = true;
    // Back from Google / Discord with an answer: that page finishes the sign-in itself.
    if (pathname === '/oauth' && (code || error)) return;
    if ((stage === 'in' || stage === 'room') && wasOut.current) {
      wasOut.current = false;
      if (pendingPlayToken() && pathname !== '/play') return void router.replace('/play');
    }
    // A party's join link and big screen work at every stage (a guest has no account yet).
    const open = pathname.startsWith('/party/join/') || pathname.startsWith('/party/screen/');
    // /play only with a shared question to show: the router also falls back to it when the guarded screens swap.
    // (After the intro: it ends on /play, see wasOut above.)
    const play = pathname === '/play' && stage !== 'verify' && stage !== 'intro' && !!(share || pendingPlayToken());
    // An invite link opened while signed in: the router drops /join and lands on home; from there, on to the join form with the code filled in.
    // Opened signed out (or before any room): signing in or /setup uses that code, not this.
    if (stage !== 'in') presetUsed = true;
    if (stage === 'in' && pathname === '/join' && (code || (!presetUsed && presetJoinCode()))) {
      setPendingCode(code ?? presetJoinCode());
      invited.current = true;
      presetUsed = true;
    }
    // The links shared before 0.32.1 (`/?code=`) land straight on home: once, the same way.
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
