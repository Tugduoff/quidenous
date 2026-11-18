import { CircleCheck, Sparkles } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { type ChoosePerson as Person, verdict } from '@/lib/utils/choose';

export function Verdict({ p, compact }: { p: Person; compact?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const v = verdict(p);
  const saw = p.real == null ? 0 : p.guesses.filter((g) => g.item === p.real).length;
  const [Icon, color] = v === 'match' ? [CircleCheck, t.accInk] : v === 'surprise' ? [Sparkles, t.tagInk] : [null, t.mut];
  if (compact) return Icon ? <Icon size={18} color={color} /> : null;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 6 }}>
      {Icon && <Icon size={16} color={color} />}
      <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color }}>
        {tr(`results.choose.${v}`)}
      </Text>
      {p.guesses.length > 0 && p.real != null && (
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {` · ${tr('results.choose.saw', { n: saw, total: p.guesses.length })}`}
        </Text>
      )}
    </View>
  );
}
