import { router } from 'expo-router';
import { ArrowRight } from 'lucide-react-native';
import { useState } from 'react';
import { TextInput } from 'react-native';
import { Card } from '@/components/ui/card';
import { PressableScale } from '@/components/ui/pressable-scale';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

/** A bad or ended link: another code to try. */
export function OtherCode() {
  const t = useTheme();
  const tr = useT();
  const [other, setOther] = useState('');
  const otherCode = other.trim().toUpperCase();
  const tryOther = () => {
    if (otherCode.length < 4) return;
    setOther('');
    router.replace(`/party/join/${encodeURIComponent(otherCode)}`);
  };
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16, marginTop: 8 }}>
      <TextInput
        accessibilityLabel={tr('party.join.other')}
        placeholder={tr('party.join.other')}
        placeholderTextColor={t.mut}
        value={other}
        onChangeText={(v) => setOther(v.replace(/[^a-z0-9]/gi, '').toUpperCase())}
        maxLength={12}
        autoCapitalize="characters"
        autoCorrect={false}
        returnKeyType="go"
        onSubmitEditing={tryOther}
        style={{ flex: 1, minWidth: 0, height: 48, fontFamily: t.font.bold, fontSize: 17, letterSpacing: other ? 2 : 0, color: t.txt }}
      />
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={tr('party.join.go')}
        disabled={otherCode.length < 4}
        onPress={tryOther}
        style={{
          width: 44,
          height: 44,
          borderRadius: 22,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: otherCode.length >= 4 ? t.acc : t.card2,
        }}>
        <ArrowRight size={20} color={otherCode.length >= 4 ? t.accTxt : t.mut} />
      </PressableScale>
    </Card>
  );
}
