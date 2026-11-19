import { Moon, Sun, SunMoon } from 'lucide-react-native';
import { Pressable, View } from 'react-native';
import { type TKey, useT } from '@/lib/i18n';
import { ACCENTS, setAccent, setThemePref, useAccent, useTheme, useThemePref, type Accent, type ThemePref } from '@/lib/theme';

const THEMES: { pref: ThemePref; Icon: typeof Sun; label: TKey }[] = [
  { pref: 'system', Icon: SunMoon, label: 'profile.theme.system' },
  { pref: 'light', Icon: Sun, label: 'profile.theme.light' },
  { pref: 'dark', Icon: Moon, label: 'profile.theme.dark' },
];

export function ThemePicker() {
  const t = useTheme();
  const tr = useT();
  const pref = useThemePref();
  return (
  <View style={{ flexDirection: 'row', gap: 10 }}>
    {THEMES.map(({ pref: p, Icon, label }) => {
      const on = p === pref;
      return (
        <Pressable
          key={p}
          accessibilityRole="radio"
          accessibilityLabel={tr(label)}
          accessibilityState={{ selected: on }}
          onPress={() => setThemePref(p)}
          style={[
            { flex: 1, height: 56, borderRadius: 18, alignItems: 'center', justifyContent: 'center', borderWidth: 2 },
            { backgroundColor: on ? t.accSoft : t.card, borderColor: on ? t.accInk : 'transparent' },
            t.shadow,
          ]}>
          <Icon size={22} color={on ? t.accInk : t.mut} />
        </Pressable>
      );
    })}
  </View>
  );
}

export function AccentPicker() {
  const t = useTheme();
  const tr = useT();
  const accent = useAccent();
  return (
  <View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
    {(Object.keys(ACCENTS) as Accent[]).map((k) => {
      const on = k === accent;
      return (
        <Pressable
          key={k}
          accessibilityRole="radio"
          accessibilityLabel={tr('profile.accentA11y', { name: tr(`profile.accents.${k}`) })}
          accessibilityState={{ selected: on }}
          onPress={() => setAccent(k)}
          style={{
            width: 44,
            height: 44,
            borderRadius: 22,
            alignItems: 'center',
            justifyContent: 'center',
            borderWidth: 2,
            borderColor: on ? t.txt : 'transparent',
          }}>
          <View style={{ width: 32, height: 32, borderRadius: 16, backgroundColor: ACCENTS[k].acc }} />
        </Pressable>
      );
    })}
  </View>
  );
}
