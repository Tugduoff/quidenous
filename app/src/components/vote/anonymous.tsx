import { Glasses } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Segmented } from '@/components/ui/segmented';
import type { AnonymousMode } from '@/lib/api/group';

export function AnonymousBadge() {
  const t = useTheme();
  const tr = useT();
  return (
    <View
      accessible
      accessibilityLabel={tr('anonymous.a11y')}
      style={{
        flexDirection: 'row',
        alignItems: 'center',
        gap: 5,
        height: 26,
        paddingHorizontal: 10,
        borderRadius: t.radius.pill,
        backgroundColor: t.card2,
      }}>
      <Glasses size={14} color={t.mut} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>{tr('anonymous.badge')}</Text>
    </View>
  );
}

export function AnonymousMark({ size = 14 }: { size?: number }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View accessible accessibilityLabel={tr('anonymous.a11y')}>
      <Glasses size={size} color={t.mut} />
    </View>
  );
}

export function AnonymousSetting({ value, onChange }: { value: AnonymousMode; onChange: (v: AnonymousMode) => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, minHeight: 24 }}>
        <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('anonymous.setting')}</Text>
        <Glasses size={18} color={value === 'never' ? t.mut : t.accInk} />
      </View>
      <Segmented
        fill
        label={tr('anonymous.setting')}
        value={value}
        onChange={onChange}
        options={[
          { id: 'never', name: tr('anonymous.never') },
          { id: 'per_question', name: tr('anonymous.perQuestion') },
          { id: 'always', name: tr('anonymous.always') },
        ]}
      />
    </View>
  );
}

export function AnonymousToggle({ value, onChange }: { value: boolean; onChange: (v: boolean) => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Pressable
      accessibilityRole="switch"
      accessibilityState={{ checked: value }}
      accessibilityLabel={tr('anonymous.toggle')}
      onPress={() => onChange(!value)}
      hitSlop={6}
      style={{
        width: 36,
        height: 36,
        borderRadius: 18,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: value ? t.accSoft : t.card2,
        borderWidth: 1.5,
        borderColor: value ? t.accInk : 'transparent',
      }}>
      <Glasses size={18} color={value ? t.accInk : t.mut} />
    </Pressable>
  );
}
