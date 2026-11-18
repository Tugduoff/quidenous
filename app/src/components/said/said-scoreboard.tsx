import { Trophy } from 'lucide-react-native';
import { memo } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import type { SaidResult } from '@/lib/api/rounds';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { places } from '@/lib/utils/said';

/**
 * Points, best first; the same points share a place; my line stands out. The big screen: `k` (a size factor) and
 * `columns` (2: the list split in two, top to bottom then the second column).
 */
export function SaidScoreboard({
  scores,
  byId,
  me,
  k = 1,
  columns = 1,
}: {
  scores: SaidResult['scores'];
  byId: Map<number, Member>;
  me: number | undefined;
  k?: number;
  columns?: 1 | 2;
}) {
  const t = useTheme();
  const tr = useT();
  const list = scores.filter((s) => byId.has(s.member_id));
  const at = places(list);
  const nobody = !list.some((s) => s.points > 0);
  const row = (s: SaidResult['scores'][number], i: number) => (
    <ScoreRow key={s.member_id} score={s} member={byId.get(s.member_id)} place={at[i]} i={i} mine={s.member_id === me} k={k} />
  );
  const half = Math.ceil(list.length / 2);
  return (
    <View style={{ gap: 10 * k }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: Math.round(17 * k), color: t.txt }}>{tr('said.result.board')}</Text>
      <Card style={{ padding: 10 * k, gap: 4 * k }}>
        {nobody && (
          <Text style={{ padding: 6 * k, fontFamily: t.font.regular, fontSize: Math.round(14 * k), color: t.mut }}>{tr('said.result.nobody')}</Text>
        )}
        {columns === 2 && list.length > 1 ? (
          <View style={{ flexDirection: 'row', gap: 12 * k }}>
            <View style={{ flex: 1, gap: 4 * k }}>{list.slice(0, half).map(row)}</View>
            <View style={{ flex: 1, gap: 4 * k }}>{list.slice(half).map((s, i) => row(s, half + i))}</View>
          </View>
        ) : (
          list.map(row)
        )}
      </Card>
    </View>
  );
}

const ScoreRow = memo(function ScoreRow({
  score: s,
  member: m,
  place,
  i,
  mine,
  k,
}: {
  score: SaidResult['scores'][number];
  member: Member | undefined;
  place: number;
  i: number;
  mine: boolean;
  k: number;
}) {
  const t = useTheme();
  const tr = useT();
  const top = place === 1 && s.points > 0;
  const f = (n: number) => Math.round(n * k);
  return (
    <Animated.View
      entering={motion.stagger(i)}
      accessible
      accessibilityLabel={tr('said.result.row', {
        place,
        name: mine ? `${m?.name ?? '?'} (${tr('said.result.you')})` : (m?.name ?? '?'),
        points: s.points,
        found: tr('said.result.found', { count: s.found }),
        fooled: tr('said.result.fooled', { count: s.fooled }),
      })}
      style={{
        flexDirection: 'row',
        alignItems: 'center',
        gap: 10 * k,
        minHeight: 56 * k,
        paddingHorizontal: 10 * k,
        borderRadius: 16 * k,
        backgroundColor: mine ? t.accSoft : 'transparent',
      }}>
      <Text style={{ width: 22 * k, textAlign: 'center', fontFamily: t.font.display, fontSize: f(16), color: top ? t.accInk : t.mut }}>{place}</Text>
      <Avatar member={m} size={f(36)} />
      <View style={{ flex: 1, minWidth: 0, gap: 1 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 * k }}>
          <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: f(15), color: t.txt }}>
            {m?.name ?? '?'}
          </Text>
          {top && <Trophy size={f(14)} color={m?.color ?? t.accInk} />}
          {mine && <Text style={{ fontFamily: t.font.semibold, fontSize: f(12), color: t.accInk }}>{tr('said.result.you')}</Text>}
        </View>
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: f(12), color: t.mut }}>
          {tr('said.result.found', { count: s.found })} · {tr('said.result.fooled', { count: s.fooled })}
        </Text>
      </View>
      <Text style={{ fontFamily: t.font.display, fontSize: f(22), color: s.points ? t.txt : t.mut }}>
        {s.points}
        <Text style={{ fontFamily: t.font.semibold, fontSize: f(12), color: t.mut }}> {tr('results.pts')}</Text>
      </Text>
    </Animated.View>
  );
});
