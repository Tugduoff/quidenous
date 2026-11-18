import { Pencil, Quote } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { AnswerText } from '@/components/said/answer-text';
import { Label } from '@/components/said/label';
import { Card } from '@/components/ui/card';
import { PressableScale } from '@/components/ui/pressable-scale';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

/** My answer once sent: anonymous for the others until the guessing, rewritable until then (`onEdit`). */
export function MyAnswer({ text, onEdit }: { text: string; onEdit?: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Card style={{ padding: 16, gap: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <Quote size={14} color={t.accInk} fill={t.accInk} />
        <View style={{ flex: 1 }}>
          <Label>{tr('said.write.yours')}</Label>
        </View>
        {onEdit && (
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('said.write.editA11y')}
            onPress={onEdit}
            hitSlop={6}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 6,
              height: 36,
              paddingHorizontal: 14,
              borderRadius: t.radius.pill,
              backgroundColor: t.card2,
            }}>
            <Pencil size={14} color={t.txt} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{tr('said.write.edit')}</Text>
          </PressableScale>
        )}
      </View>
      <AnswerText answer={{ text, hidden: false }} size={19} />
    </Card>
  );
}
