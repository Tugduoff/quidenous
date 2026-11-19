import { Globe } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { tidy } from '@/lib/utils/text';
import { useTheme } from '@/lib/theme';
import { Display } from '@/components/ui/text';

export function GlobalHeading({ question }: { question: string }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
        <Globe size={15} color={t.accInk} />
        <Text style={{ fontFamily: t.font.bold, fontSize: 12, letterSpacing: 0.6, color: t.accInk }}>{tr('globalQuestion.label').toUpperCase()}</Text>
      </View>
      <Display size={30}>{tidy(question)}</Display>
    </View>
  );
}
