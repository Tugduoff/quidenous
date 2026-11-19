import { Pressable, Text, View } from 'react-native';
import Animated, { FadeIn, ReduceMotion } from 'react-native-reanimated';
import { Verdict } from '@/components/choose/verdict';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { type ChoosePerson as Person, verdict } from '@/lib/utils/choose';

const sys = ReduceMotion.System;

export function ChooseSummary({ people, items, byId, onOne }: { people: Person[]; items: string[]; byId: Map<number, Member>; onOne: () => void }) {
  const t = useTheme();
  const tr = useT();
  const name = (k: number | null | undefined) => (k == null ? '?' : (items[k] ?? '?'));
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      {people.map((p, i) => {
        const m = byId.get(p.member_id);
        const v = verdict(p);
        const guess = p.top.map((k) => items[k]).join(' / ');
        return (
          <Animated.View
            key={p.member_id}
            entering={FadeIn.delay(Math.min(i, 8) * 40)
              .duration(200)
              .reduceMotion(sys)}
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
            <Avatar member={m} size={32} />
            <View style={{ flex: 1, minWidth: 0 }}>
              <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
                {m?.name ?? '?'}
              </Text>
              <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
                {v === 'surprise' ? (
                  <>
                    {`${guess} → `}
                    <Text style={{ fontFamily: t.font.semibold, color: t.txt }}>{name(p.real)}</Text>
                  </>
                ) : v === 'match' ? (
                  <Text style={{ fontFamily: t.font.semibold, color: t.txt }}>{name(p.real)}</Text>
                ) : v === 'noGuess' ? (
                  <>
                    <Text style={{ fontFamily: t.font.semibold, color: t.txt }}>{name(p.real)}</Text>
                    {` · ${tr('results.choose.noGuess')}`}
                  </>
                ) : (
                  tr('results.choose.noAnswer')
                )}
              </Text>
            </View>
            <Verdict p={p} compact />
          </Animated.View>
        );
      })}
      <Pressable
        accessibilityRole="button"
        onPress={onOne}
        hitSlop={8}
        style={{ alignSelf: 'center', minHeight: 40, justifyContent: 'center', paddingHorizontal: 12 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('results.choose.oneByOne')}</Text>
      </Pressable>
    </Card>
  );
}
