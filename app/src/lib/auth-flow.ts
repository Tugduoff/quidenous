import { presetJoinCode } from '@/lib/device/launch-params';
import { Platform } from 'react-native';

/** Between the sign-in screens (not in the URL): the email typed, the 2FA challenge. */
export const flow = { email: '', account: false, challenge: '', mailFailed: false };

/** An invite code (link or deep link) opened before signing in: joined right after. */
let pendingCode: string | null = presetJoinCode();

export const setPendingCode = (code: string | null) => {
  pendingCode = code?.trim().toUpperCase() || null;
};

export const pendingJoinCode = () => pendingCode;

/**
 * A shared question to play (`/play?share=…`) opened before signing in: back on /play right after.
 * On the web it is read at load (the router drops the query) and kept in the tab's session storage,
 * so leaving for Google / Discord and coming back keeps it.
 */
const PLAY_KEY = 'qdn.play';

const webSession = () => (Platform.OS === 'web' && typeof sessionStorage !== 'undefined' ? sessionStorage : null);

let pendingPlay: string | null = (() => {
  if (Platform.OS !== 'web' || typeof location === 'undefined') return null;
  const fromUrl = location.pathname.replace(/\/+$/, '') === '/play' ? new URLSearchParams(location.search).get('share') : null;
  if (fromUrl) webSession()?.setItem(PLAY_KEY, fromUrl);
  return fromUrl || webSession()?.getItem(PLAY_KEY) || null;
})();

export const setPendingPlay = (token: string | null) => {
  pendingPlay = token?.trim() || null;
  if (pendingPlay) webSession()?.setItem(PLAY_KEY, pendingPlay);
  else webSession()?.removeItem(PLAY_KEY);
};

export const pendingPlayToken = () => pendingPlay;
