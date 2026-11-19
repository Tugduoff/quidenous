export const FONT = {
  display: 'BricolageGrotesque_800ExtraBold',
  regular: 'Inter_400Regular',
  semibold: 'Inter_600SemiBold',
  bold: 'Inter_700Bold',
};

export const ACCENTS = {
  lime: { acc: '#C8FF3D', inkDark: '#C8FF3D', inkLight: '#477000', softDark: '#1F2412', softLight: '#F1FFD0' },
  rose: { acc: '#FF6DB5', inkDark: '#FF6DB5', inkLight: '#C2186B', softDark: '#2A1220', softLight: '#FFE3F0' },
  bleu: { acc: '#5AC8FA', inkDark: '#5AC8FA', inkLight: '#0A6FA8', softDark: '#0F2230', softLight: '#DFF4FF' },
  orange: { acc: '#FFB547', inkDark: '#FFB547', inkLight: '#9A5B00', softDark: '#2A1E0B', softLight: '#FFF0D6' },
  violet: { acc: '#B48CFF', inkDark: '#C2A3FF', inkLight: '#6A3FD0', softDark: '#1E1830', softLight: '#EFE7FF' },
  menthe: { acc: '#5EF2B0', inkDark: '#5EF2B0', inkLight: '#0B7A4F', softDark: '#0F2A20', softLight: '#DDFBEE' },
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
    tagBg: COLORS.tagFill,
    tagTxt: COLORS.onStrong,
    shadow: {},
  },
  light: {
    ...shared,
    bg: '#F5F5F0',
    card: '#FFFFFF',
    card2: '#ECECE5',
    line: '#E2E2DA',
    txt: '#121216',
    mut: '#64646D',
    accInk: '#477000',
    accSoft: '#F1FFD0',
    tag: '#E8338A',
    tagInk: '#C2186B',
    tagBg: COLORS.tagFill,
    tagTxt: COLORS.onStrong,
    shadow: {
      shadowColor: COLORS.lightShadow,
      shadowOpacity: 0.06,
      shadowRadius: 10,
      shadowOffset: { width: 0, height: 2 },
      elevation: 2,
    },
  },
};

type RoomTokens = {
  bg: string;
  card: string;
  card2: string;
  line: string;
  txt: string;
  mut: string;
  acc: string;
  accTxt: string;
  accInk: string;
  accSoft: string;
  tagBg: string;
  tagTxt: string;
};

export type ArtPalette = {
  sky: readonly [string, string];
  skyA: readonly [number, number];
  far?: string;
  a: string;
  b: string;
  c: string;
  ink: string;
};

type RoomLook = { dark: RoomTokens; light: RoomTokens; art: { dark: ArtPalette; light: ArtPalette } };

const INK = COLORS.ink;

export const ROOM_THEMES = {
  apero: {
    dark: { bg: '#170B0A', card: '#261513', card2: '#33201C', line: '#412B26', txt: '#FBF4F1', mut: '#B89C95', acc: '#FF7A52', accTxt: INK, accInk: '#FF8E6B', accSoft: '#3A1A12', tagBg: '#D6E04F', tagTxt: INK },
    light: { bg: '#FFF4EC', card: '#FFFFFF', card2: '#F9E6DA', line: '#F0D8C9', txt: '#24120E', mut: '#75594F', acc: '#FF7A52', accTxt: INK, accInk: '#B03409', accSoft: '#FFE3D6', tagBg: '#CBD93E', tagTxt: INK },
    art: {
      dark: { sky: ['#6B1E3C', '#C4471F'], skyA: [0.55, 0.95], a: '#FFC94D', b: '#FF7A52', c: '#FF5D8F', ink: INK },
      light: { sky: ['#FFD3B8', '#FF9E7A'], skyA: [0.6, 1], a: '#FFC233', b: '#FF6A3D', c: '#FF5D8F', ink: INK },
    },
  },
  feu: {
    dark: { bg: '#07130F', card: '#0F211A', card2: '#182D25', line: '#223A30', txt: '#F2F8F4', mut: '#8FA99D', acc: '#FFC53D', accTxt: INK, accInk: '#FFC53D', accSoft: '#2B2610', tagBg: '#FF7A45', tagTxt: INK },
    light: { bg: '#F1F7EF', card: '#FFFFFF', card2: '#E2EEE2', line: '#D3E3D5', txt: '#0E1A14', mut: '#43574C', acc: '#FFC53D', accTxt: INK, accInk: '#7F5300', accSoft: '#FFF1C9', tagBg: '#FF7A45', tagTxt: INK },
    art: {
      dark: { sky: ['#0E3B3A', '#1D5A4A'], skyA: [0.6, 0.9], far: '#12302A', a: '#FFC53D', b: '#FF7A2E', c: '#FF5A3C', ink: INK },
      light: { sky: ['#CDEBD9', '#FFE2A8'], skyA: [0.7, 1], far: '#A9D6B8', a: '#FFC53D', b: '#FF7A2E', c: '#F0522A', ink: INK },
    },
  },
  karaoke: {
    dark: { bg: '#130A1C', card: '#21132D', card2: '#2D1D3B', line: '#3A2849', txt: '#F8F4FB', mut: '#BCA9CD', acc: '#FF5FD2', accTxt: INK, accInk: '#FF7ADB', accSoft: '#3A1238', tagBg: '#4DE8FF', tagTxt: INK },
    light: { bg: '#FAF3FD', card: '#FFFFFF', card2: '#F1E5F7', line: '#E6D7EE', txt: '#1C1222', mut: '#594A66', acc: '#FF5FD2', accTxt: INK, accInk: '#AE1293', accSoft: '#FFE0F7', tagBg: '#3FDDF5', tagTxt: INK },
    art: {
      dark: { sky: ['#4A1466', '#A3197F'], skyA: [0.55, 0.9], a: '#FF5FD2', b: '#4DE8FF', c: '#FFE066', ink: INK },
      light: { sky: ['#E9CCFF', '#FFB3EA'], skyA: [0.6, 1], a: '#F03CC0', b: '#12B5D0', c: '#FFC933', ink: INK },
    },
  },
  pyjama: {
    dark: { bg: '#0B0C1E', card: '#161832', card2: '#202341', line: '#2B2E50', txt: '#F4F5FC', mut: '#9BA0C6', acc: '#ABA3FF', accTxt: INK, accInk: '#B9B2FF', accSoft: '#22214A', tagBg: '#FFD86B', tagTxt: INK },
    light: { bg: '#F5F4FF', card: '#FFFFFF', card2: '#E9E8FA', line: '#DDDCF2', txt: '#141527', mut: '#4F5170', acc: '#ABA3FF', accTxt: INK, accInk: '#5142CC', accSoft: '#E8E5FF', tagBg: '#FFD152', tagTxt: INK },
    art: {
      dark: { sky: ['#2A2470', '#6A3FA6'], skyA: [0.6, 0.92], far: '#221F58', a: '#FFE08A', b: '#F2B94A', c: '#FFFFFF', ink: INK },
      light: { sky: ['#D5CFFF', '#FFC9E3'], skyA: [0.65, 1], far: '#C9C2FA', a: '#FFD86B', b: '#F2B03A', c: '#FFFFFF', ink: INK },
    },
  },
  arcade: {
    dark: { bg: '#090B16', card: '#141726', card2: '#1E2134', line: '#292D43', txt: '#F3F5FB', mut: '#969DB8', acc: '#C8FF3D', accTxt: INK, accInk: '#C8FF3D', accSoft: '#1E2A12', tagBg: '#FF5FD2', tagTxt: INK },
    light: { bg: '#F2F4FA', card: '#FFFFFF', card2: '#E6E9F3', line: '#D9DDEA', txt: '#12141E', mut: '#5A5F70', acc: '#C8FF3D', accTxt: INK, accInk: '#3F6600', accSoft: '#EDFFC4', tagBg: '#FF5FD2', tagTxt: INK },
    art: {
      dark: { sky: ['#241A66', '#7A1F8C'], skyA: [0.6, 0.9], a: '#C8FF3D', b: '#FFC53D', c: '#FF5FD2', ink: INK },
      light: { sky: ['#CFD6FF', '#F2C4FF'], skyA: [0.65, 1], a: '#5E9900', b: '#FFB01F', c: '#F03CC0', ink: INK },
    },
  },
  piscine: {
    dark: { bg: '#04121C', card: '#0A1F2D', card2: '#122B3B', line: '#1B384A', txt: '#F1F8FB', mut: '#86A8BC', acc: '#3FE3D2', accTxt: INK, accInk: '#3FE3D2', accSoft: '#0C2E33', tagBg: '#FF8571', tagTxt: INK },
    light: { bg: '#EEF9FC', card: '#FFFFFF', card2: '#DDF0F5', line: '#CCE5EC', txt: '#0B1920', mut: '#4C6672', acc: '#3FE3D2', accTxt: INK, accInk: '#006F66', accSoft: '#D2F8F3', tagBg: '#FF8571', tagTxt: INK },
    art: {
      dark: { sky: ['#0B4A66', '#11808C'], skyA: [0.6, 0.92], far: '#0D3550', a: '#FFD54A', b: '#FF8571', c: '#FFFFFF', ink: INK },
      light: { sky: ['#B5ECF2', '#7FDDE3'], skyA: [0.7, 1], far: '#C4F1F5', a: '#FFD02E', b: '#FF6B55', c: '#FFFFFF', ink: INK },
    },
  },
} as const satisfies Record<string, RoomLook>;

const APERO_NIGHT = ROOM_THEMES.apero.art.dark;

export const SCENE = {
  white: '#FFFFFF',
  bean: ACCENTS.lime.acc,
  beachBall: ROOM_THEMES.piscine.dark.acc,
  log: { dark: '#8A5A32', light: '#9A6236' },
  lightsWire: { dark: '#4A2E28', light: '#8A6257' },
  lightsBulbs: { dark: [APERO_NIGHT.a, APERO_NIGHT.c, '#7CE8C4', APERO_NIGHT.b], light: ['#F5A800', '#F0437A', '#12B886', '#FF6A3D'] },
  mirrorBall: { dark: ['#FFFFFF', '#CDBBF5', '#7C61C4'], light: ['#FFFFFF', '#DCCDF8', '#A58AE0'] },
  mirrorCord: { dark: '#CDBBF5', light: '#8F78C8' },
  micHead: { dark: '#EDE6FA', light: '#F4EEFC' },
} as const;

export const SEASON = {
  snow: {
    dark: { speck: '#FFFFFF', flake: '#FFFFFF', crystal: '#FFFFFF' },
    light: { speck: '#A9C6F0', flake: '#8DB5EC', crystal: '#6FA3E6' },
  },
  drift: {
    dark: { hill: '#9FB2D1', crest: '#DCE6F5', bank: '#E4ECF8', edge: '#FFFFFF' },
    light: { hill: '#C6D6EC', crest: '#E6EEF9', bank: '#DCE7F6', edge: '#FFFFFF' },
  },
  pompom: '#FFFFFF',
} as const;
