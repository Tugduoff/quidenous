import { ChevronDown, ChevronUp, Crown } from 'lucide-react-native';
import { type ReactNode, useEffect, useState } from 'react';
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

import type { ChooseResult, Member, RankEntry, Tally } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { BOUNCE, EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

import { Avatar, AvatarStack, PressableScale } from './ui';

/** Rows shown before "Voir tout": a big group's result stays one screen tall. */
const TOP = 5;
/** Only fold the list when that hides at least two rows (hiding a single one saves nothing). */
const folded = <T,>(rows: T[], all: boolean) => (all || rows.length <= TOP + 1 ? rows : rows.slice(0, TOP));

/** Pick result: one bar per person who got votes (the top 5, then "Voir tout"). */
export function ResultBars({ result, byId }: { result: Tally[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const [all, setAll] = useState(false);
  const total = result.reduce((n, r) => n + r.count, 0) || 1;
  const top = result[0]?.count ?? 0;
  const shown = folded(result, all);
  return (
    <View style={{ gap: 12 }}>
      {shown.map((r, i) => {
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
                <Text style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 16, color: t.txt }} numberOfLines={1}>
                  {m?.name}
                </Text>
                {win && (
                  <Landed rows={Math.min(result.length, TOP)}>
                    <Crown size={18} color={color} />
                  </Landed>
                )}
              </View>
              {/* Capped so a big group's voters leave room for the name. */}
              <AvatarStack members={r.voter_ids.map((id) => byId.get(id))} size={24} max={4} />
              <Text style={{ fontFamily: t.font.display, fontSize: 22, color: t.txt }}>{pct}%</Text>
            </View>
          </Land>
        );
      })}
      {shown.length < result.length && <More count={result.length} onPress={() => setAll(true)} />}
      {all && result.length > TOP + 1 && <More less onPress={() => setAll(false)} />}
    </View>
  );
}

/** "Voir tout (N)" under a folded result, "Réduire" once it is open. */
function More({ count, less, onPress }: { count?: number; less?: boolean; onPress: () => void }) {
  const t = useTheme();
  const tr = useT();
  const Icon = less ? ChevronUp : ChevronDown;
  return (
    <PressableScale
      accessibilityRole="button"
      onPress={onPress}
      hitSlop={6}
      style={{
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 4,
        alignSelf: 'center',
        minHeight: 44,
        paddingHorizontal: 16,
      }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
        {less ? tr('results.less') : tr('results.seeAll', { count: count ?? 0 })}
      </Text>
      <Icon size={16} color={t.mut} />
    </PressableScale>
  );
}

/** Ranking result: everyone in order, with points (Borda) and how many put them first. */
export function RankResult({ ranking, byId, mine }: { ranking: RankEntry[]; byId: Map<number, Member>; mine: number[] | null }) {
  const t = useTheme();
  const tr = useT();
  const [all, setAll] = useState(false);
  const best = ranking[0]?.points || 1;
  // Gold, silver, bronze discs for the podium.
  const podium = ['#F5C542', '#C9D0DA', '#D9935A'];
  const shown = folded(ranking, all);
  return (
    <View style={{ gap: 10 }}>
      {shown.map((r, i) => {
        const m = byId.get(r.member_id);
        const color = m?.color ?? t.mut;
        const myPlace = mine ? mine.indexOf(r.member_id) + 1 : 0;
        // Past the top 5 (a big group, opened with "Voir tout"), slimmer rows.
        const slim = i >= TOP;
        return (
          <Land
            key={r.member_id}
            on={i === 0}
            style={[
              { height: slim ? 52 : 60, borderRadius: slim ? 14 : 18, overflow: 'hidden', backgroundColor: t.card, justifyContent: 'center' },
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
              <Avatar member={m} size={slim ? 28 : 36} />
              <View style={{ flex: 1 }}>
                <Text style={{ fontFamily: t.font.bold, fontSize: slim ? 15 : 16, color: t.txt }} numberOfLines={1}>
                  {m?.name}
                </Text>
                {/* Only where my ranking differs from the group's. */}
                {myPlace > 0 && myPlace !== i + 1 && (
                  <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>{tr('results.you', { place: myPlace })}</Text>
                )}
              </View>
              <Text style={{ fontFamily: t.font.display, fontSize: 20, color: t.txt }}>
                {r.points}
                <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}> {tr('results.pts')}</Text>
              </Text>
            </View>
          </Land>
        );
      })}
      {shown.length < ranking.length && <More count={ranking.length} onPress={() => setAll(true)} />}
      {all && ranking.length > TOP + 1 && <More less onPress={() => setAll(false)} />}
    </View>
  );
}

/**
 * The result in one line (winner, share or points), pinned over the top of the page once the
 * full result has scrolled away under the chat. Tap it to go back up.
 */
export function ResultSummary({
  result,
  ranking,
  choose,
  byId,
  onPress,
}: {
  result: Tally[];
  ranking: RankEntry[] | null;
  /** A choose question: its best guesser, hits out of guesses. */
  choose?: ChooseResult | null;
  byId: Map<number, Member>;
  onPress: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  const total = result.reduce((n, r) => n + r.count, 0) || 1;
  const guesser = choose?.scores[0];
  const winnerId = choose ? guesser?.member_id : ranking ? ranking[0]?.member_id : result[0]?.target_id;
  const m = winnerId != null ? byId.get(winnerId) : undefined;
  const score = choose
    ? `${guesser?.hits ?? 0}/${guesser?.total ?? 0}`
    : ranking
      ? tr('results.points', { count: ranking[0]?.points ?? 0 })
      : `${Math.round(((result[0]?.count ?? 0) / total) * 100)}%`;
  // Several people sharing first place: "+1" after the name.
  const tied = choose
    ? choose.scores.filter((s) => s.hits === guesser?.hits && s.total === guesser?.total).length - 1
    : ranking
      ? 0
      : result.filter((r) => r.count === result[0]?.count).length - 1;
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={tr('results.summary', { name: m?.name ?? '?', score })}
      onPress={onPress}
      style={[
        {
          flexDirection: 'row',
          alignItems: 'center',
          gap: 10,
          paddingVertical: 8,
          paddingLeft: 8,
          paddingRight: 14,
          borderRadius: 24,
          backgroundColor: t.card,
        },
        t.shadow,
      ]}>
      <Avatar member={m} size={32} />
      <Crown size={16} color={m?.color ?? t.mut} />
      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>
        {m?.name ?? '?'}
      </Text>
      {tied > 0 && <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>+{tied}</Text>}
      <Text style={{ marginLeft: 'auto', fontFamily: t.font.display, fontSize: 18, color: t.txt }}>{score}</Text>
      <ChevronUp size={18} color={t.mut} />
    </PressableScale>
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
