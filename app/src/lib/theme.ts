import { useColorScheme } from 'react-native';

const shared = {
  acc: '#C8FF3D',
  accTxt: '#121216',
  radius: { card: 20, pill: 999 },
  font: {
    display: 'BricolageGrotesque_800ExtraBold',
    regular: 'Inter_400Regular',
    semibold: 'Inter_600SemiBold',
    bold: 'Inter_700Bold',
  },
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
    shadow: {
      shadowColor: '#000',
      shadowOpacity: 0.06,
      shadowRadius: 10,
      shadowOffset: { width: 0, height: 2 },
      elevation: 2,
    },
  },
};

export type Theme = typeof themes.dark;

export function useTheme(): Theme {
  return themes[useColorScheme() === 'light' ? 'light' : 'dark'];
}
