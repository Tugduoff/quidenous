import { useTheme } from '@/lib/theme';
import { View, Pressable, Text } from 'react-native';

export function Segmented<T extends string>({
  label,
  options,
  value,
  onChange,
  fill,
}: {
  label: string;
  options: { id: T; name: string }[];
  value: T;
  onChange: (v: T) => void;
  fill?: boolean;
}) {
  const t = useTheme();
  return (
    <View
      accessibilityRole="radiogroup"
      accessibilityLabel={label}
      style={{ flexDirection: 'row', padding: 3, gap: 2, borderRadius: t.radius.pill, backgroundColor: t.card2 }}>
      {options.map((o) => {
        const on = o.id === value;
        return (
          <Pressable
            key={o.id}
            accessibilityRole="radio"
            accessibilityState={{ selected: on }}
            accessibilityLabel={o.name}
            onPress={() => !on && onChange(o.id)}
            hitSlop={{ top: 7, bottom: 7 }}
            style={{
              height: 30,
              paddingHorizontal: fill ? 8 : 12,
              flexGrow: fill ? 1 : 0,
              borderRadius: t.radius.pill,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: on ? t.acc : 'transparent',
            }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: on ? t.accTxt : t.mut }}>{o.name}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}
