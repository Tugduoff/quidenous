import { Quote } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated, { FadeIn, FadeInDown, ReduceMotion } from 'react-native-reanimated';
import { AnswerText } from '@/components/said/answer-text';
import { Flip } from '@/components/said/flip';
import { GuessRow } from '@/components/said/guess-row';
import { Label } from '@/components/said/label';
import { PointsPill } from '@/components/said/points-pill';
import type { Member } from '@/lib/api/group';
import { useT } from '@/lib/i18n';
import { LAYOUT_EASE, motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { FLIP_MS, flipAt, GUESS_IN, GUESS_STEP, guessGroups, type RevealItem, screenGroups } from '@/lib/utils/said';

const sys = ReduceMotion.System;

/** Answer `step` of the reveal, big: the progress, the answer, who was named, then the author flips. `k`: a size factor. */
export function SaidScreenReveal({
  items,
  step,
  byId,
  k,
  wide,
}: {
  items: RevealItem[];
  step: number;
  byId: Map<number, Member>;
  k: number;
  /** The answer on the left, the names and the author on the right. */
  wide: boolean;
}) {
  const t = useTheme();
  const tr = useT();
  const item = items[Math.min(step, items.length - 1)];
  if (!item) return null;
  return (
    <View style={{ gap: 28 * k }}>
      <View
        accessible
        accessibilityLabel={tr('said.reveal.step', { n: step + 1, total: items.length })}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 8 * k }}>
        {items.map((it, i) => (
          <View key={it.id} style={{ flex: 1, maxWidth: 72 * k, height: 6 * k, borderRadius: 3 * k, backgroundColor: i <= step ? t.accInk : t.card2 }} />
        ))}
        <View style={{ flex: 1 }} />
        <Text style={{ fontFamily: t.font.display, fontSize: Math.round(22 * k), color: t.mut, fontVariant: ['tabular-nums'] }}>
          {tr('said.reveal.step', { n: step + 1, total: items.length })}
        </Text>
      </View>
      <ScreenRevealStep key={item.id} item={item} byId={byId} k={k} wide={wide} />
    </View>
  );
}

function ScreenRevealStep({ item, byId, k, wide }: { item: RevealItem; byId: Map<number, Member>; k: number; wide: boolean }) {
  const t = useTheme();
  const tr = useT();
  const all = guessGroups(item);
  const at = flipAt(all.length);
  const { groups, rest } = screenGroups(all);
  const author = byId.get(item.author_id);
  const f = (n: number) => Math.round(n * k);
  const long = (item.text?.length ?? 0) > 80;
  return (
    <View style={{ flexDirection: wide ? 'row' : 'column', alignItems: wide ? 'center' : 'stretch', gap: 48 * k }}>
      <Animated.View entering={motion.question} style={{ flex: wide ? 1.15 : undefined, flexDirection: 'row', gap: 16 * k }}>
        <Quote size={f(40)} color={t.accInk} fill={t.accInk} style={{ marginTop: 10 * k }} />
        <View style={{ flex: 1 }}>
          {item.text != null ? (
            <Text
              style={{
                fontFamily: t.font.display,
                fontSize: f(long ? 46 : 60),
                lineHeight: f(long ? 52 : 66),
                letterSpacing: -1,
                color: t.txt,
              }}>
              {item.text}
            </Text>
          ) : (
            <AnswerText answer={item} size={f(32)} />
          )}
        </View>
      </Animated.View>
      <View style={{ flex: wide ? 1 : undefined, gap: 12 * k }}>
        <Label k={1.9 * k}>{tr('said.reveal.named')}</Label>
        {groups.length ? (
          groups.map((g, i) => (
            <GuessRow key={g.member_id} group={g} byId={byId} delay={GUESS_IN + i * GUESS_STEP} settle={at + FLIP_MS} k={1.75 * k} />
          ))
        ) : (
          <Text style={{ fontFamily: t.font.regular, fontSize: f(26), color: t.mut }}>{tr('said.reveal.noGuess')}</Text>
        )}
        {rest > 0 && <Text style={{ fontFamily: t.font.semibold, fontSize: f(22), color: t.mut }}>{tr('said.party.more', { count: rest })}</Text>}
        <View style={{ height: 1, marginVertical: 6 * k, backgroundColor: t.line }} />
        <View
          accessible
          accessibilityLabel={`${tr('said.reveal.it')} ${author?.name ?? '?'}. ${tr('said.reveal.fooled', { count: item.fooled })}. ${tr('said.reveal.found', { count: item.found })}`}
          style={{ flexDirection: 'row', alignItems: 'center', gap: 20 * k }}>
          <Flip member={author} at={at} size={f(120)} />
          <View style={{ flex: 1, gap: 4 * k }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: f(26), color: t.mut }}>{tr('said.reveal.it')}</Text>
            <Animated.Text
              entering={FadeIn.delay(at + FLIP_MS / 2)
                .duration(160)
                .reduceMotion(sys)}
              numberOfLines={1}
              style={{ fontFamily: t.font.display, fontSize: f(58), lineHeight: f(64), letterSpacing: -1.5, color: t.txt }}>
              {author?.name ?? '?'}
            </Animated.Text>
          </View>
          <Animated.View
            entering={FadeInDown.delay(at + FLIP_MS)
              .duration(260)
              .easing(LAYOUT_EASE)
              .reduceMotion(sys)}>
            <PointsPill points={item.fooled} k={1.9 * k} />
          </Animated.View>
        </View>
        <Animated.Text
          entering={FadeIn.delay(at + FLIP_MS + 180)
            .duration(240)
            .reduceMotion(sys)}
          style={{ fontFamily: t.font.semibold, fontSize: f(24), lineHeight: f(32), color: t.mut }}>
          {tr('said.reveal.fooled', { count: item.fooled })} · {tr('said.reveal.found', { count: item.found })}
        </Animated.Text>
      </View>
    </View>
  );
}
