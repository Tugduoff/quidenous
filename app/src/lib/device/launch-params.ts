import { Platform } from 'react-native';

// Read at load time: the router rewrites the URL (and drops the query) once it mounts.
const INVITE_CODE =
  Platform.OS === 'web' && typeof location !== 'undefined'
    ? new URLSearchParams(location.search).get('code')
    : null;

const FRIEND_CODE =
  Platform.OS === 'web' && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('friend') : null;

const ROOM_PARAM =
  Platform.OS === 'web' && typeof location !== 'undefined' ? Number(new URLSearchParams(location.search).get('room')) || null : null;

/** Room carried by a tapped notification (`/round/12?room=3`): the app opens in it. */
export const presetRoom = () => ROOM_PARAM;

// The bare home page (the installed web app adds `?source=pwa`): no link or notification chose a screen (lib/today.ts).
const PLAIN_START =
  Platform.OS === 'web' && typeof location !== 'undefined'
    ? location.pathname.replace(/\/+$/, '') === '' && [...new URLSearchParams(location.search).keys()].every((k) => k === 'source')
    : false;

/** Web: the page was opened at `/` without a link's parameters. */
export const plainStart = () => PLAIN_START;

/** Friend code carried by a friend link `/?friend=…` (QR code). */
export const presetFriendCode = () => FRIEND_CODE;

/** Group code baked into the build (CI secret) or carried by the invite link `/?code=…`. */
export function presetJoinCode(): string | null {
  // Rooms are open to anyone now: only an invite link fills the code in (no group baked into the build).
  return INVITE_CODE;
}
