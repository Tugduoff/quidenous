import { Check, X } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { AnswerText } from '@/components/said/answer-text';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { nameOf } from '@/lib/utils/names';
import type { RevealItem } from '@/lib/utils/said';

export function SaidAnswerList({ items, byId }: { items: RevealItem[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>{tr('said.result.list')}</Text>
      <Card style={{ padding: 12, gap: 8 }}>
        {items.map((it) => {
          const m = byId.get(it.author_id);
          const verdict = it.own || it.myGuess == null ? null : it.myGuess === it.author_id;
          return (
            <View
              key={it.id}
              accessible
              accessibilityLabel={`${tr('said.reveal.author', { name: m?.name ?? '?', text: it.text ?? tr('said.guess.hidden') })}. ${tr('said.reveal.fooled', { count: it.fooled })}. ${tr('said.reveal.found', { count: it.found })}${verdict == null ? '' : `. ${verdict ? tr('said.reveal.right') : tr('said.reveal.wrong', { name: nameOf(byId, it.myGuess) })}`}`}
              style={{ padding: 12, gap: 10, borderRadius: 14, backgroundColor: t.card2 }}>
              <AnswerText answer={it} size={15} />
              <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                <Avatar member={m} size={24} />
                <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 14, color: t.txt }}>
                  {m?.name ?? '?'}
                  {it.own && <Text style={{ fontFamily: t.font.regular, color: t.mut }}>{` · ${tr('said.result.you')}`}</Text>}
                </Text>
                <View style={{ flex: 1 }} />
                <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>
                  {tr('said.result.fooled', { count: it.fooled })} · {tr('said.result.found', { count: it.found })}
                </Text>
                {verdict != null &&
                  (verdict ? <Check size={16} color={t.accInk} strokeWidth={3} /> : <X size={16} color={t.tagInk} strokeWidth={3} />)}
              </View>
            </View>
          );
        })}
      </Card>
    </View>
  );
}
