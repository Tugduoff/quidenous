import { ArrowRight, Check, Quote, Trophy, X } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated, { FadeIn, FadeInDown, ReduceMotion, useReducedMotion } from 'react-native-reanimated';
import { AnswerText } from '@/components/said/answer-text';
import { Flip } from '@/components/said/flip';
import { GuessRow } from '@/components/said/guess-row';
import { Label } from '@/components/said/label';
import { PointsPill } from '@/components/said/points-pill';
import { Card } from '@/components/ui/card';
import { PressableScale } from '@/components/ui/pressable-scale';
import type { Member } from '@/lib/api/group';
import { impactHaptic } from '@/lib/device/haptics';
import { useTimeout } from '@/lib/hooks/use-timeout';
import { useT } from '@/lib/i18n';
import { LAYOUT_EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { nameOf } from '@/lib/utils/names';
import { FLIP_MS, flipAt, GUESS_IN, GUESS_STEP, guessGroups, type RevealItem } from '@/lib/utils/said';

const sys = ReduceMotion.System;

export function SaidReveal({ items, byId, onDone }: { items: RevealItem[]; byId: Map<number, Member>; onDone: () => void }) {
  const t = useTheme();
  const tr = useT();
  const [step, setStep] = useState(0);
  const item = items[Math.min(step, items.length - 1)];
  if (!item) return null;
  const last = step >= items.length - 1;
  return (
    <Card style={{ padding: 18, gap: 16 }}>
      <View
        style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}
        accessibilityLabel={tr('said.reveal.step', { n: step + 1, total: items.length })}>
        {items.map((it, i) => (
          <View key={it.id} style={{ flex: 1, maxWidth: 40, height: 4, borderRadius: 2, backgroundColor: i <= step ? t.accInk : t.card2 }} />
        ))}
        <View style={{ flex: 1 }} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, fontVariant: ['tabular-nums'] }}>
          {tr('said.reveal.step', { n: step + 1, total: items.length })}
        </Text>
      </View>
      <RevealStep key={item.id} item={item} byId={byId} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        {!last && (
          <Pressable
            accessibilityRole="button"
            onPress={onDone}
            hitSlop={8}
            style={{ minHeight: 44, justifyContent: 'center', paddingHorizontal: 6 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('said.reveal.all')}</Text>
          </Pressable>
        )}
        <View style={{ flex: 1 }} />
        <PressableScale
          accessibilityRole="button"
          onPress={() => (last ? onDone() : setStep(step + 1))}
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 8,
            height: 46,
            paddingHorizontal: 20,
            borderRadius: t.radius.pill,
            backgroundColor: last ? t.acc : t.card2,
          }}>
          {last && <Trophy size={16} color={t.accTxt} />}
          <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: last ? t.accTxt : t.txt }}>
            {tr(last ? 'said.reveal.scores' : 'said.reveal.next')}
          </Text>
          {!last && <ArrowRight size={16} color={t.txt} />}
        </PressableScale>
      </View>
    </Card>
  );
}

function RevealStep({ item, byId }: { item: RevealItem; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const groups = guessGroups(item);
  const at = flipAt(groups.length);
  const author = byId.get(item.author_id);
  const reduced = useReducedMotion();
  useTimeout(impactHaptic, reduced ? null : at + FLIP_MS / 2);
  return (
    <View style={{ gap: 16 }}>
      <View style={{ flexDirection: 'row', gap: 10 }}>
        <Quote size={22} color={t.accInk} fill={t.accInk} style={{ marginTop: 3 }} />
        <View style={{ flex: 1 }}>
          {item.text != null ? (
            <Text style={{ fontFamily: t.font.display, fontSize: 24, lineHeight: 29, letterSpacing: -0.5, color: t.txt }}>{item.text}</Text>
          ) : (
            <AnswerText answer={item} size={18} />
          )}
        </View>
      </View>
      <View style={{ gap: 8 }}>
        <Label>{tr('said.reveal.named')}</Label>
        {groups.length ? (
          groups.map((g, i) => <GuessRow key={g.member_id} group={g} byId={byId} delay={GUESS_IN + i * GUESS_STEP} settle={at + FLIP_MS} />)
        ) : (
          <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>{tr('said.reveal.noGuess')}</Text>
        )}
      </View>
      <View style={{ height: 1, backgroundColor: t.line }} />
      <View
        accessible
        accessibilityLabel={`${tr('said.reveal.it')} ${author?.name ?? '?'}. ${tr('said.reveal.fooled', { count: item.fooled })}. ${tr('said.reveal.found', { count: item.found })}`}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 14 }}>
        <Flip member={author} at={at} />
        <View style={{ flex: 1, gap: 4 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('said.reveal.it')}</Text>
          <Animated.Text
            entering={FadeIn.delay(at + FLIP_MS / 2)
              .duration(160)
              .reduceMotion(sys)}
            numberOfLines={1}
            style={{ fontFamily: t.font.display, fontSize: 24, letterSpacing: -0.5, color: t.txt }}>
            {author?.name ?? '?'}
          </Animated.Text>
        </View>
        <Animated.View
          entering={FadeInDown.delay(at + FLIP_MS)
            .duration(260)
            .easing(LAYOUT_EASE)
            .reduceMotion(sys)}>
          <PointsPill points={item.fooled} />
        </Animated.View>
      </View>
      <Animated.View
        entering={FadeIn.delay(at + FLIP_MS + 180)
          .duration(240)
          .reduceMotion(sys)}
        style={{ gap: 6 }}>
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 19, color: t.mut }}>
          {tr('said.reveal.fooled', { count: item.fooled })} · {tr('said.reveal.found', { count: item.found })}
        </Text>
        <MyVerdict item={item} byId={byId} />
      </Animated.View>
    </View>
  );
}

function MyVerdict({ item, byId }: { item: RevealItem; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  if (!item.own && item.myGuess == null) return null;
  const right = item.myGuess === item.author_id;
  const [Icon, color, text] = item.own
    ? [Quote, t.accInk, tr('said.reveal.own')]
    : right
      ? [Check, t.accInk, tr('said.reveal.right')]
      : [X, t.tagInk, tr('said.reveal.wrong', { name: nameOf(byId, item.myGuess) })];
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
      <Icon size={16} color={color} strokeWidth={3} />
      <Text style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 15, color }}>{text}</Text>
    </View>
  );
}
