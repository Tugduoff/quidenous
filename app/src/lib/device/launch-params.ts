import { Platform } from 'react-native';

import { parseOverride } from '@/lib/utils/season';

const INVITE_CODE =
  Platform.OS === 'web' && typeof location !== 'undefined'
    ? new URLSearchParams(location.search).get('code')
    : null;

const FRIEND_CODE =
  Platform.OS === 'web' && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('friend') : null;

const ROOM_PARAM =
  Platform.OS === 'web' && typeof location !== 'undefined' ? Number(new URLSearchParams(location.search).get('room')) || null : null;

export const presetRoom = () => ROOM_PARAM;

const PLAIN_START =
  Platform.OS === 'web' && typeof location !== 'undefined'
    ? location.pathname.replace(/\/+$/, '') === '' && [...new URLSearchParams(location.search).keys()].every((k) => k === 'source')
    : false;

export const plainStart = () => PLAIN_START;

export const presetFriendCode = () => FRIEND_CODE;

export function presetJoinCode(): string | null {
  return INVITE_CODE;
}

const SEASON_OVERRIDE = Platform.OS === 'web' && typeof location !== 'undefined' ? parseOverride(location.search) : null;

export const presetSeason = () => SEASON_OVERRIDE;
