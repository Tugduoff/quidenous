import { Crown } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { SectionTitle } from '@/components/choose/section-title';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import type { ChooseResult as ChooseData } from '@/lib/api/rounds';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

/** Who guessed best: hits out of the guesses that could hit. */
export function BestGuessers({ scores, byId }: { scores: ChooseData['scores']; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const [more, setMore] = useState(false);
  const list = scores.filter((s) => s.total > 0 && byId.has(s.member_id));
  if (!list.length) return null;
  const shown = more || list.length <= 6 ? list : list.slice(0, 5);
  const best = list[0].hits;
  return (
    <View style={{ gap: 10 }}>
      <SectionTitle>{tr('results.choose.best')}</SectionTitle>
      <Card style={{ padding: 12, gap: 6 }}>
        {shown.map((s, i) => {
          const m = byId.get(s.member_id);
          return (
            <View key={s.member_id} style={{ flexDirection: 'row', alignItems: 'center', gap: 10, minHeight: 40 }}>
              <Text style={{ width: 20, textAlign: 'center', fontFamily: t.font.display, fontSize: 15, color: t.mut }}>{i + 1}</Text>
              <Avatar member={m} size={30} />
              <View style={{ flex: 1, minWidth: 0, flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                  {m?.name ?? '?'}
                </Text>
                {best > 0 && s.hits === best && <Crown size={15} color={m?.color ?? t.mut} />}
              </View>
              <Text style={{ fontFamily: t.font.display, fontSize: 17, color: t.txt }}>
                {s.hits}
                <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>/{s.total}</Text>
              </Text>
            </View>
          );
        })}
        {shown.length < list.length && (
          <Pressable
            accessibilityRole="button"
            onPress={() => setMore(true)}
            hitSlop={8}
            style={{ alignSelf: 'center', minHeight: 40, justifyContent: 'center' }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('results.seeAll', { count: list.length })}</Text>
          </Pressable>
        )}
      </Card>
    </View>
  );
}
