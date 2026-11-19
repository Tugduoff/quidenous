import { EyeOff } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import type { SaidAnswer } from '@/lib/api/rounds';

export function AnswerText({ answer, size = 17 }: { answer: Pick<SaidAnswer, 'text' | 'hidden'>; size?: number }) {
  const t = useTheme();
  const tr = useT();
  if (answer.text == null)
    return (
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, minHeight: size * 1.35 }}>
        <EyeOff size={size - 1} color={t.mut} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: size - 2, color: t.mut, fontStyle: 'italic' }}>{tr('said.guess.hidden')}</Text>
      </View>
    );
  return <Text style={{ fontFamily: t.font.semibold, fontSize: size, lineHeight: Math.round(size * 1.35), color: t.txt }}>{answer.text}</Text>;
}
