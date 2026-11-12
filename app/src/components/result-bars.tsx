import { Crown } from 'lucide-react-native';
import { type ReactNode, useEffect } from 'react';
import { type StyleProp, Text, View, type ViewStyle } from 'react-native';
import Animated, {
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withDelay,
  withSequence,
  withSpring,
  withTiming,
} from 'react-native-reanimated';

import type { Member, RankEntry, Tally } from '@/lib/api';
import { BOUNCE, EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

import { Avatar, AvatarStack } from './ui';

export function ResultBars({ result, byId }: { result: Tally[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const total = result.reduce((n, r) => n + r.count, 0) || 1;
  const top = result[0]?.count ?? 0;
  return (
    <View style={{ gap: 12 }}>
      {result.map((r, i) => {
        const m = byId.get(r.target_id);
        const color = m?.color ?? t.mut;
        const win = r.count === top;
        const pct = Math.round((r.count / total) * 100);
        return (
          <Land
            key={r.target_id}
            on={win}
            style={[
              { height: 64, borderRadius: 18, overflow: 'hidden', backgroundColor: t.card, justifyContent: 'center' },
              t.shadow,
              win && { borderWidth: 2, borderColor: color },
            ]}>
            <Fill index={i} width={pct} color={color} opacity={win ? 0.55 : 0.28} />
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingHorizontal: 12 }}>
              <Avatar member={m} />
              <View style={{ flex: 1, flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }} numberOfLines={1}>
                  {m?.name}
                </Text>
                {win && (
                  <Landed rows={result.length}>
                    <Crown size={18} color={color} />
                  </Landed>
                )}
              </View>
              <AvatarStack members={r.voter_ids.map((id) => byId.get(id))} size={24} />
              <Text style={{ fontFamily: t.font.display, fontSize: 22, color: t.txt }}>{pct}%</Text>
            </View>
          </Land>
        );
      })}
    </View>
  );
}

/** Ranking result: everyone in order, with points (Borda) and how many put them first. */
export function RankResult({ ranking, byId, mine }: { ranking: RankEntry[]; byId: Map<number, Member>; mine: number[] | null }) {
  const t = useTheme();
  const best = ranking[0]?.points || 1;
  // Gold, silver, bronze discs for the podium.
  const podium = ['#F5C542', '#C9D0DA', '#D9935A'];
  return (
    <View style={{ gap: 10 }}>
      {ranking.map((r, i) => {
        const m = byId.get(r.member_id);
        const color = m?.color ?? t.mut;
        const myPlace = mine ? mine.indexOf(r.member_id) + 1 : 0;
        return (
          <Land
            key={r.member_id}
            on={i === 0}
            style={[
              { height: 60, borderRadius: 18, overflow: 'hidden', backgroundColor: t.card, justifyContent: 'center' },
              t.shadow,
              i === 0 && { borderWidth: 2, borderColor: color },
            ]}>
            <Fill index={i} width={Math.round((r.points / best) * 100)} color={color} opacity={i === 0 ? 0.5 : 0.25} />
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingHorizontal: 12 }}>
              <View
                style={{
                  width: 30,
                  height: 30,
                  borderRadius: 15,
                  alignItems: 'center',
                  justifyContent: 'center',
                  backgroundColor: podium[i] ?? 'transparent',
                }}>
                <Text style={{ fontFamily: t.font.display, fontSize: i < 3 ? 15 : 18, color: i < 3 ? '#1A1A1A' : t.txt }}>{i + 1}</Text>
              </View>
              <Avatar member={m} size={36} />
              <View style={{ flex: 1 }}>
                <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }} numberOfLines={1}>
                  {m?.name}
                </Text>
                {/* Only where my ranking differs from the group's. */}
                {myPlace > 0 && myPlace !== i + 1 && (
                  <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>{`toi : ${myPlace}e`}</Text>
                )}
              </View>
              <Text style={{ fontFamily: t.font.display, fontSize: 20, color: t.txt }}>
                {r.points}
                <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}> pts</Text>
              </Text>
            </View>
          </Land>
        );
      })}
    </View>
  );
}

// Bars start 120 ms in, each row 70 ms after the previous one, and take 450 ms to fill.
const fillDelay = (i: number) => 120 + Math.min(i, 6) * 70;
const FILL_MS = 450;

/** A result row; the winner's lands with a small bump once its own bar is full. */
function Land({ on, style, children }: { on: boolean; style: StyleProp<ViewStyle>; children: ReactNode }) {
  const reduced = useReducedMotion();
  const s = useSharedValue(1);
  useEffect(() => {
    if (!on || reduced) return;
    s.value = withDelay(fillDelay(0) + FILL_MS - 60, withSequence(withTiming(1.03, { duration: 110, easing: EASE }), withSpring(1, BOUNCE)));
  }, [on, reduced, s]);
  const bump = useAnimatedStyle(() => ({ transform: [{ scale: s.value }] }));
  return <Animated.View style={[style, bump]}>{children}</Animated.View>;
}

/** The crown: pops in once every bar has finished growing. */
function Landed({ rows, children }: { rows: number; children: ReactNode }) {
  const reduced = useReducedMotion();
  const s = useSharedValue(reduced ? 1 : 0);
  useEffect(() => {
    s.value = withDelay(fillDelay(rows - 1) + FILL_MS, withSpring(1, BOUNCE));
  }, [rows, s]);
  const pop = useAnimatedStyle(() => ({ opacity: Math.min(s.value, 1), transform: [{ scale: s.value }] }));
  return <Animated.View style={pop}>{children}</Animated.View>;
}

/** A row's coloured fill: grows from the left when the result shows, one row after the other. */
function Fill({ index, width, color, opacity }: { index: number; width: number; color: string; opacity: number }) {
  const reduced = useReducedMotion();
  const grow = useSharedValue(reduced ? 1 : 0);
  useEffect(() => {
    grow.value = withDelay(fillDelay(index), withTiming(1, { duration: FILL_MS, easing: EASE }));
  }, [grow, index]);
  // A transform rather than the width: no layout pass per frame.
  const style = useAnimatedStyle(() => ({ transform: [{ scaleX: grow.value }] }));
  return (
    <Animated.View
      style={[
        { position: 'absolute', left: 0, top: 0, bottom: 0, width: `${width}%`, backgroundColor: color, opacity, transformOrigin: 'left' },
        style,
      ]}
    />
  );
}
