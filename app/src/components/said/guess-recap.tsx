import { ArrowRight } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { AnswerText } from '@/components/said/answer-text';
import { Label } from '@/components/said/label';
import { NumberDisc } from '@/components/said/number-disc';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import type { SaidAnswer, SaidGuess } from '@/lib/api/rounds';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { firstName } from '@/lib/utils/names';

export function GuessRecap({ answers, guesses, byId }: { answers: SaidAnswer[]; guesses: SaidGuess[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const said = new Map(guesses.map((g) => [g.answer_id, g.member_id]));
  const rows = answers.filter((a) => said.has(a.id));
  if (!rows.length) return null;
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      <Label>{tr('said.recap.title')}</Label>
      {rows.map((a, i) => {
        const author = said.get(a.id);
        const m = author == null ? undefined : byId.get(author);
        return (
          <View
            key={a.id}
            accessible
            accessibilityLabel={`${a.text ?? tr('said.guess.hidden')}. ${tr('said.guess.on', { name: m?.name ?? '?' })}`}
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
            <NumberDisc n={i + 1} />
            <View style={{ flex: 1 }}>
              {a.text != null ? (
                <Text numberOfLines={2} style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}>
                  {a.text}
                </Text>
              ) : (
                <AnswerText answer={a} size={15} />
              )}
            </View>
            <ArrowRight size={14} color={t.mut} />
            <View style={{ alignItems: 'center', width: 56, gap: 2 }}>
              <Avatar member={m} size={30} />
              <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 11, color: t.mut }}>
                {m ? firstName(m.name) : '?'}
              </Text>
            </View>
          </View>
        );
      })}
    </Card>
  );
}
