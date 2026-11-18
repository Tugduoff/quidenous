import { Pressable, ScrollView, Text } from 'react-native';
import type { Field, PaletteField } from '@/lib/utils/avatar';
import { type TKey, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

type Tab = { label: TKey; field?: Field; colors?: PaletteField; zoom?: boolean };
export const TABS: Tab[] = [
  { label: 'avatar.tabs.presets' },
  { label: 'avatar.tabs.face', field: 'face', colors: 'skin' },
  { label: 'avatar.tabs.hair', field: 'hair', colors: 'hairColor' },
  { label: 'avatar.tabs.eyes', field: 'eyes', zoom: true },
  { label: 'avatar.tabs.brows', field: 'brows', zoom: true },
  { label: 'avatar.tabs.mouth', field: 'mouth', zoom: true },
  { label: 'avatar.tabs.glasses', field: 'glasses', zoom: true },
  { label: 'avatar.tabs.beard', field: 'beard', colors: 'hairColor', zoom: true },
  { label: 'avatar.tabs.extra', field: 'extra' },
  { label: 'avatar.tabs.background', colors: 'bg' },
];

export function AvatarTabs({ tab, onTab }: { tab: number; onTab: (i: number) => void }) {
  const t = useTheme();
  const tr = useT();
  return (
  <ScrollView
    horizontal
    showsHorizontalScrollIndicator={false}
    style={{ marginHorizontal: -20, flexGrow: 0 }}
    contentContainerStyle={{ paddingHorizontal: 20, gap: 8 }}>
    {TABS.map((tb, i) => {
      const on = i === tab;
      return (
        <Pressable
          key={tb.label}
          accessibilityRole="tab"
          accessibilityState={{ selected: on }}
          onPress={() => onTab(i)}
          style={{ height: 38, paddingHorizontal: 16, borderRadius: 19, justifyContent: 'center', backgroundColor: on ? t.acc : t.card }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: on ? t.accTxt : t.txt }}>{tr(tb.label)}</Text>
        </Pressable>
      );
    })}
  </ScrollView>
  );
}
