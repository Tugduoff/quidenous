export const FONT = {
  display: 'BricolageGrotesque_800ExtraBold',
  regular: 'Inter_400Regular',
  semibold: 'Inter_600SemiBold',
  bold: 'Inter_700Bold',
};

export const ACCENTS = {
  lime: { acc: '#C8FF3D', inkDark: '#C8FF3D', inkLight: '#4F7D00', softDark: '#1F2412', softLight: '#F1FFD0' },
  rose: { acc: '#FF6DB5', inkDark: '#FF6DB5', inkLight: '#C2186B', softDark: '#2A1220', softLight: '#FFE3F0' },
  bleu: { acc: '#5AC8FA', inkDark: '#5AC8FA', inkLight: '#0A6FA8', softDark: '#0F2230', softLight: '#DFF4FF' },
  orange: { acc: '#FFB547', inkDark: '#FFB547', inkLight: '#B26A00', softDark: '#2A1E0B', softLight: '#FFF0D6' },
  violet: { acc: '#B48CFF', inkDark: '#C2A3FF', inkLight: '#6A3FD0', softDark: '#1E1830', softLight: '#EFE7FF' },
  menthe: { acc: '#5EF2B0', inkDark: '#5EF2B0', inkLight: '#0F8A5A', softDark: '#0F2A20', softLight: '#DDFBEE' },
} as const;
export type Accent = keyof typeof ACCENTS;

export const MEDALS = {
  1: { key: 'bronze', fill: '#D9935A', light: '#F4C79F', rim: '#9A5A2A', inkLight: '#8A4B1C', inkDark: '#E8A673' },
  2: { key: 'silver', fill: '#C3CBD6', light: '#F2F5F9', rim: '#7D8794', inkLight: '#56606E', inkDark: '#D5DCE5' },
  3: { key: 'gold', fill: '#F2C230', light: '#FFE89A', rim: '#B07F00', inkLight: '#805C00', inkDark: '#F5C542' },
} as const;

export const COLORS = {
  ink: '#121216',
  inkSoft: '#121216B3',
  inkTrack: '#1212162E',
  inkFaint: '#1212161F',
  onStrong: '#FFFFFF',
  gloss: '#FFFFFF',
  paperSoft: '#FFFFFF8C',
  danger: '#FF3B5C',
  dangerFill: '#D7263D',
  tagFill: '#C2186B',
  tagSoft: '#FF4FA318',
  gold: '#F2B705',
  coin: { faces: ['#FFD54A', '#FFC107', '#F2B705', '#FFE082'], edge: '#B8860B' },
  podium: ['#F5C542', '#C9D0DA', '#D9935A'],
  qr: { ink: '#121216', paper: '#FFFFFF' },
  media: { backdrop: '#000000', on: '#FFFFFF', scrimLight: 'rgba(0,0,0,0.45)', scrim: 'rgba(0,0,0,0.5)', scrimStrong: 'rgba(0,0,0,0.55)' },
  dialogScrim: { anchored: '#0006', centered: '#0008' },
  mentionOnMine: 'rgba(0,0,0,0.12)',
  mentionFallback: 'rgba(127,127,127,0.18)',
  wireShadow: '#000000',
  liftShadow: '0 12px 28px rgba(0, 0, 0, 0.35)',
  lightShadow: '#000000',
};

const DEFAULT_ACCENT: string = ACCENTS.lime.acc;

const shared = {
  acc: DEFAULT_ACCENT,
  accTxt: COLORS.ink,
  ...COLORS,
  radius: { card: 20, pill: 999 },
  font: FONT,
};

export const themes = {
  dark: {
    ...shared,
    bg: '#0E0E12',
    card: '#1A1A21',
    card2: '#24242D',
    line: '#2E2E38',
    txt: '#F5F5F7',
    mut: '#8D8D99',
    accInk: '#C8FF3D',
    accSoft: '#1F2412',
    tag: '#FF4FA3',
    tagInk: '#FF4FA3',
    shadow: {},
  },
  light: {
    ...shared,
    bg: '#F5F5F0',
    card: '#FFFFFF',
    card2: '#ECECE5',
    line: '#E2E2DA',
    txt: '#121216',
    mut: '#6E6E78',
    accInk: '#4F7D00',
    accSoft: '#F1FFD0',
    tag: '#E8338A',
    tagInk: '#C2186B',
    shadow: {
      shadowColor: COLORS.lightShadow,
      shadowOpacity: 0.06,
      shadowRadius: 10,
      shadowOffset: { width: 0, height: 2 },
      elevation: 2,
    },
  },
};
