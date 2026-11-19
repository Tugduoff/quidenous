import { Pressable, View } from 'react-native';
import { PARTS, type PaletteField } from '@/lib/utils/avatar';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function AvatarSwatches({ field, value, onPick }: { field: PaletteField; value: number; onPick: (i: number) => void }) {
  const t = useTheme();
  const tr = useT();
  const big = field === 'bg';
  const outer = big ? 52 : 44;
  return (
    <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: big ? 10 : 6 }}>
      {PARTS[field].map(([c], i) => {
        const on = i === value;
        return (
          <Pressable
            key={c}
            accessibilityRole="radio"
            accessibilityLabel={tr('avatar.color', { n: i + 1 })}
            accessibilityState={{ selected: on }}
            onPress={() => onPick(i)}
            style={{
              width: outer,
              height: outer,
              borderRadius: outer / 2,
              alignItems: 'center',
              justifyContent: 'center',
              borderWidth: 2,
              borderColor: on ? t.txt : 'transparent',
            }}>
            <View style={{ width: outer - 12, height: outer - 12, borderRadius: outer, backgroundColor: c }} />
          </Pressable>
        );
      })}
    </View>
  );
}
