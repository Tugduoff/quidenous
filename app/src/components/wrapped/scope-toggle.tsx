import { Pressable, Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import type { WrappedScope } from '@/lib/api/wrapped';
import { INK } from '@/components/wrapped/constants';

export function ScopeToggle({ value, onChange, ink, track }: { value: WrappedScope; onChange: (s: WrappedScope) => void; ink: string; track: string }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View accessibilityRole="radiogroup" accessibilityLabel={tr('wrapped.scope.label')} style={{ flexDirection: 'row', padding: 3, gap: 2, borderRadius: 999, backgroundColor: track }}>
      {(['me', 'room'] as const).map((s) => {
        const on = s === value;
        return (
          <Pressable
            key={s}
            accessibilityRole="radio"
            accessibilityState={{ selected: on }}
            accessibilityLabel={tr(`wrapped.scope.${s}`)}
            onPress={() => !on && onChange(s)}
            hitSlop={{ top: 7, bottom: 7 }}
            style={{ height: 30, paddingHorizontal: 14, borderRadius: 999, alignItems: 'center', justifyContent: 'center', backgroundColor: on ? ink : 'transparent' }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: on ? (ink === INK ? '#FFFFFF' : t.bg) : ink }}>{tr(`wrapped.scope.${s}`)}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}
