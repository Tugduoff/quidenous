import { ArrowDown, ArrowRight } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { ScrollView } from 'react-native-gesture-handler';
import Svg, { Circle, G, Rect, Text as SvgText } from 'react-native-svg';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import type { Stats } from '@/lib/api/stats';

const GAP = 2;
const MIN = 12;
const MAX = 40;

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
  const tr = useT();
  const [width, setWidth] = useState(0);
  const count = new Map(edges.map((e) => [`${e.voter_id}:${e.target_id}`, e.value]));
  const max = Math.max(1, ...edges.filter((e) => ids.includes(e.voter_id) && ids.includes(e.target_id)).map((e) => e.value));
  const n = ids.length;
  const HEAD = n > 12 ? 22 : 28;
  const cell = width ? Math.max(MIN, Math.min(MAX, Math.floor((width - HEAD - GAP) / n) - GAP)) : 0;
  const face = Math.min(cell, 28);
  const showCounts = cell >= 24;
  const step = cell + GAP;
  const side = n * step - GAP;

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
      <View style={{ flexDirection: 'row', gap: GAP }}>
        <View style={{ width: HEAD, gap: GAP }}>
          {ids.map((voter) => (
            <Pressable
              key={voter}
              accessibilityRole="button"
              accessibilityLabel={byId.get(voter)?.name}
              onPress={() => onOpen(voter)}
              hitSlop={2}
              style={{ width: HEAD, height: cell, alignItems: 'center', justifyContent: 'center' }}>
              <Avatar member={byId.get(voter)} size={Math.min(HEAD, Math.max(face, 20), cell)} />
            </Pressable>
          ))}
        </View>
        <Svg width={side} height={side}>
          {ids.flatMap((voter, row) =>
            ids.map((target, col) => {
              const c = count.get(`${voter}:${target}`) ?? 0;
              const self = voter === target;
              if (!c && !self) return <Rect key={`${voter}:${target}`} x={col * step} y={row * step} width={cell} height={cell} rx={Math.min(8, cell / 4)} fill={t.card2} />;
              const color = byId.get(target)?.color ?? t.mut;
              const strength = c ? 0.2 + 0.8 * (c / max) : 0;
              const x = col * step;
              const y = row * step;
              return (
                <G key={`${voter}:${target}`}>
                  {self ? (
                    <Circle cx={x + cell / 2} cy={y + cell / 2} r={cell / 2 - (c ? 0 : 0.75)} fill={c ? color : 'none'} fillOpacity={strength} stroke={c ? 'none' : t.line} strokeWidth={1.5} />
                  ) : (
                    <Rect x={x} y={y} width={cell} height={cell} rx={Math.min(8, cell / 4)} fill={color} fillOpacity={strength} />
                  )}
                  {showCounts && c > 0 && (
                    <SvgText
                      x={x + cell / 2}
                      y={y + cell / 2 + cell * 0.15}
                      fontSize={Math.round(cell * 0.42)}
                      fontFamily={t.font.bold}
                      textAnchor="middle"
                      fill={strength > 0.55 ? '#121216' : t.txt}>
                      {c}
                    </SvgText>
                  )}
                </G>
              );
            }),
          )}
        </Svg>
      </View>
    </View>
  );

  const fits = HEAD + GAP + n * (cell + GAP) <= width;
  return (
    <Card style={{ padding: 14, gap: 12 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
          <ArrowDown size={13} color={t.mut} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>{tr('stats.matrix.voter')}</Text>
        </View>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
          <ArrowRight size={13} color={t.mut} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>{tr('stats.matrix.target')}</Text>
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
