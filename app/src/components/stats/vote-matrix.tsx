import { ArrowDown, ArrowRight } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';

import { Avatar, Card } from '@/components/ui';
import type { Member, Stats } from '@/lib/api';
import { useTheme } from '@/lib/theme';

const GAP = 2;
const MIN = 12;
const MAX = 40;

/**
 * Who votes for whom: a row per voter, a column per person voted for, each cell in the target's colour,
 * stronger the more votes. The diagonal (votes for oneself) is drawn as circles. Shrinks to fit the width,
 * and scrolls sideways past a cell of 12 px (around 22 people on a small phone). Under 16 px the column heads are plain dots.
 */
export function VoteMatrix({
  ids,
  edges,
  byId,
  onOpen,
}: {
  ids: number[];
  edges: Stats['edges'];
  byId: Map<number, Member>;
  onOpen: (id: number) => void;
}) {
  const t = useTheme();
  const [width, setWidth] = useState(0);
  const count = new Map(edges.map((e) => [`${e.voter_id}:${e.target_id}`, e.value]));
  const max = Math.max(1, ...edges.filter((e) => ids.includes(e.voter_id) && ids.includes(e.target_id)).map((e) => e.value));
  const n = ids.length;
  const HEAD = n > 12 ? 22 : 28;
  const cell = width ? Math.max(MIN, Math.min(MAX, Math.floor((width - HEAD - GAP) / n) - GAP)) : 0;
  const face = Math.min(cell, 28);
  const showCounts = cell >= 24;

  const grid = (
    <View style={{ gap: GAP }}>
      <View style={{ flexDirection: 'row', gap: GAP, paddingLeft: HEAD + GAP }}>
        {ids.map((id) => (
          <Pressable
            key={id}
            accessibilityRole="button"
            accessibilityLabel={byId.get(id)?.name}
            onPress={() => onOpen(id)}
            style={{ width: cell, alignItems: 'center' }}>
            {cell >= 16 ? (
              <Avatar member={byId.get(id)} size={face} />
            ) : (
              <View style={{ width: cell - 2, height: cell - 2, borderRadius: cell, backgroundColor: byId.get(id)?.color ?? t.mut }} />
            )}
          </Pressable>
        ))}
      </View>
      {ids.map((voter) => (
        <View key={voter} style={{ flexDirection: 'row', alignItems: 'center', gap: GAP }}>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={byId.get(voter)?.name}
            onPress={() => onOpen(voter)}
            hitSlop={2}
            style={{ width: HEAD, alignItems: 'center' }}>
            <Avatar member={byId.get(voter)} size={Math.min(HEAD, Math.max(face, 20))} />
          </Pressable>
          {ids.map((target) => {
            const c = count.get(`${voter}:${target}`) ?? 0;
            const self = voter === target;
            const strength = c ? 0.2 + 0.8 * (c / max) : 0;
            const color = byId.get(target)?.color ?? t.mut;
            return (
              <View
                key={target}
                accessibilityLabel={c ? `${byId.get(voter)?.name} pour ${byId.get(target)?.name} : ${c}` : undefined}
                style={{
                  width: cell,
                  height: cell,
                  borderRadius: self ? cell / 2 : Math.min(8, cell / 4),
                  overflow: 'hidden',
                  alignItems: 'center',
                  justifyContent: 'center',
                  backgroundColor: c || self ? 'transparent' : t.card2,
                  borderWidth: self && !c ? 1.5 : 0,
                  borderColor: t.line,
                }}>
                {c > 0 && <View style={{ position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: color, opacity: strength }} />}
                {showCounts && c > 0 && (
                  <Text style={{ fontFamily: t.font.bold, fontSize: Math.round(cell * 0.42), color: strength > 0.55 ? '#121216' : t.txt }}>{c}</Text>
                )}
              </View>
            );
          })}
        </View>
      ))}
    </View>
  );

  const fits = HEAD + GAP + n * (cell + GAP) <= width;
  return (
    <Card style={{ padding: 14, gap: 12 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
          <ArrowDown size={13} color={t.mut} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>qui vote</Text>
        </View>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
          <ArrowRight size={13} color={t.mut} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>pour qui</Text>
        </View>
      </View>
      <View onLayout={(e) => setWidth(e.nativeEvent.layout.width)}>
        {width > 0 &&
          (fits ? (
            <View style={{ alignItems: 'center' }}>{grid}</View>
          ) : (
            <ScrollView horizontal showsHorizontalScrollIndicator={false}>
              {grid}
            </ScrollView>
          ))}
      </View>
    </Card>
  );
}
