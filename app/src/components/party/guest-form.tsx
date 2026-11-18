import { Check } from 'lucide-react-native';
import { Pressable, Text, TextInput, View } from 'react-native';
import { TermsCheck } from '@/components/auth/terms';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PARTS } from '@/lib/utils/avatar';

export const GUEST_COLORS = PARTS.bg.map(([c]) => c).filter((c) => c.toUpperCase() !== '#2E2E38');

export function GuestForm({
  name,
  onName,
  color,
  onColor,
  terms,
  onTerms,
  onSubmit,
}: {
  name: string;
  onName: (name: string) => void;
  color: string;
  onColor: (color: string) => void;
  terms: boolean;
  onTerms: (on: boolean) => void;
  onSubmit: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 16, marginTop: 8 }}>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 8, paddingLeft: 10 }}>
        <Avatar member={{ id: 0, name: name.trim() || '?', color, photo_url: null }} size={44} />
        <TextInput
          accessibilityLabel={tr('party.join.name')}
          placeholder={tr('party.join.name')}
          placeholderTextColor={t.mut}
          value={name}
          onChangeText={onName}
          maxLength={24}
          autoComplete="given-name"
          autoCapitalize="words"
          returnKeyType="go"
          onSubmitEditing={onSubmit}
          style={{ flex: 1, minWidth: 0, height: 48, fontFamily: t.font.semibold, fontSize: 17, color: t.txt }}
        />
      </Card>
      <View style={{ gap: 10 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{tr('party.join.color')}</Text>
        <View accessibilityRole="radiogroup" style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8 }}>
          {GUEST_COLORS.map((c, i) => {
            const on = c === color;
            return (
              <Pressable
                key={c}
                accessibilityRole="radio"
                accessibilityLabel={tr('party.join.colorA11y', { n: i + 1 })}
                accessibilityState={{ selected: on }}
                onPress={() => onColor(c)}
                style={{
                  width: 48,
                  height: 48,
                  borderRadius: 24,
                  alignItems: 'center',
                  justifyContent: 'center',
                  borderWidth: 2,
                  borderColor: on ? t.txt : 'transparent',
                }}>
                <View style={{ width: 36, height: 36, borderRadius: 18, backgroundColor: c, alignItems: 'center', justifyContent: 'center' }}>
                  {on && <Check size={18} color="#121216" />}
                </View>
              </Pressable>
            );
          })}
        </View>
      </View>
      <TermsCheck value={terms} onChange={onTerms} />
    </View>
  );
}
