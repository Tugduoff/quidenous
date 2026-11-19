import type { Theme } from '@/lib/theme';
import { themes } from '@/lib/tokens';

export const party = (t: Theme) =>
  [t.acc, '#FF4FA3', '#5AC8FA', '#FFB547', '#B48CFF', t.bg === themes.light.bg ? '#121216' : '#FFFFFF'].filter((c, i, xs) => xs.indexOf(c) === i);
export const GOLD = ['#FFD54A', '#FFB300', '#FFF1B8', '#E8A317', '#FFE082'] as const;

export const PIXEL = ['#FFE600', '#FF2A6D', '#05D9E8', '#3DFF6E', '#B48CFF', '#FF7A45'];

export const TRICOLORE = ['#2F6BFF', '#FFFFFF', '#FF4757'] as const;
export const NAVY = '#0B1026';

export const isDark = (t: Theme) => parseInt(t.bg.slice(1, 3), 16) < 128;
