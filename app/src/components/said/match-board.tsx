import { X } from 'lucide-react-native';
import { memo, useCallback } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion, useSharedValue, withRepeat, withSequence, withTiming } from 'react-native-reanimated';
import { AnswerText } from '@/components/said/answer-text';
import { NumberDisc } from '@/components/said/number-disc';
import { Avatar } from '@/components/ui/avatar';
import { PressableScale } from '@/components/ui/pressable-scale';
import type { Member } from '@/lib/api/group';
import type { SaidAnswer } from '@/lib/api/rounds';
import { selectionHaptic } from '@/lib/device/haptics';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';
import { useT } from '@/lib/i18n';
import { EASE, motion, usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { firstName } from '@/lib/utils/names';
import type { Match, MatchAction } from '@/lib/utils/said';

/** Every answer to place, as cards: tap one, then a person in the tray. Long-press (or the screen reader's action) reports. */
export function MatchBoard({
  answers,
  state,
  byId,
  dispatch,
  onReport,
}: {
  /** In the order shown (`state.order`). */
  answers: SaidAnswer[];
  state: Match;
  byId: Map<number, Member>;
  dispatch: (a: MatchAction) => void;
  /** Long-press on an answer: report it (not on a hidden one). */
  onReport?: (answerId: number) => void;
}) {
  const onSelect = useCallback(
    (id: number) => {
      selectionHaptic();
      dispatch({ type: 'select', answer: id });
    },
    [dispatch],
  );
  const onClear = useCallback((id: number) => dispatch({ type: 'clear', answer: id }), [dispatch]);
  return (
    <View style={{ gap: 12 }}>
      {answers.map((a, i) => (
        <AnswerCard
          key={a.id}
          answer={a}
          n={i + 1}
          total={answers.length}
          member={state.picks[a.id] != null ? byId.get(state.picks[a.id]) : undefined}
          selected={state.sel === a.id}
          onSelect={onSelect}
          onClear={onClear}
          onReport={onReport}
        />
      ))}
    </View>
  );
}

const AnswerCard = memo(function AnswerCard({
  answer,
  n,
  total,
  member,
  selected,
  onSelect,
  onClear,
  onReport,
}: {
  answer: SaidAnswer;
  n: number;
  total: number;
  member: Member | undefined;
  selected: boolean;
  onSelect: (id: number) => void;
  onClear: (id: number) => void;
  onReport?: (id: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const bump = usePop(member?.id, 1.025, member != null);
  const report = onReport && answer.text != null ? () => onReport(answer.id) : undefined;
  const text = answer.text ?? tr('said.guess.hidden');
  return (
    <Animated.View style={bump}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        accessibilityState={{ selected }}
        accessibilityLabel={`${tr('said.guess.a11y', { n, total, text })}. ${member ? tr('said.guess.on', { name: member.name }) : tr('said.guess.open')}`}
        accessibilityHint={selected ? tr('said.guess.pickNow') : undefined}
        accessibilityActions={[
          ...(member ? [{ name: 'clear', label: tr('said.guess.clear', { name: member.name }) }] : []),
          ...(report ? [{ name: 'report', label: tr('moderation.reportAnswer') }] : []),
        ]}
        onAccessibilityAction={(e) => {
          if (e.nativeEvent.actionName === 'clear') onClear(answer.id);
          if (e.nativeEvent.actionName === 'report') report?.();
        }}
        onPress={() => onSelect(answer.id)}
        onLongPress={report}
        delayLongPress={450}
        style={[
          {
            padding: 16,
            gap: 14,
            borderRadius: t.radius.card,
            borderWidth: 2,
            borderColor: selected ? t.accInk : 'transparent',
            backgroundColor: selected ? t.accSoft : t.card,
          },
          t.shadow,
        ]}>
        <View style={{ flexDirection: 'row', gap: 12, alignItems: 'flex-start' }}>
          <NumberDisc n={n} on={selected} />
          <View style={{ flex: 1, paddingTop: 1 }}>
            <AnswerText answer={answer} />
          </View>
        </View>
        <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'flex-end', gap: 8, minHeight: 36 }}>
          {member ? <Signature member={member} selected={selected} onClear={() => onClear(answer.id)} /> : <WhoSlot waiting={selected} />}
        </View>
      </PressableScale>
    </Animated.View>
  );
});

/** "Qui ?" in a dashed pill; breathes while the card waits for a person. */
function WhoSlot({ waiting }: { waiting: boolean }) {
  const t = useTheme();
  const tr = useT();
  const reduced = useReducedMotion();
  const s = useSharedValue(1);
  useAnimateOnChange(s, [waiting, reduced], () =>
    waiting && !reduced
      ? withRepeat(withSequence(withTiming(1.07, { duration: 520, easing: EASE }), withTiming(1, { duration: 520, easing: EASE })), -1)
      : withTiming(1, { duration: 120 }),
  );
  const breathe = useAnimatedStyle(() => ({ transform: [{ scale: s.value }] }));
  return (
    <Animated.View
      style={[
        {
          flexDirection: 'row',
          alignItems: 'center',
          gap: 6,
          height: 36,
          paddingLeft: 4,
          paddingRight: 12,
          borderRadius: t.radius.pill,
          borderWidth: 1.5,
          borderStyle: 'dashed',
          borderColor: waiting ? t.accInk : t.mut,
        },
        breathe,
      ]}>
      <View style={{ width: 28, height: 28, borderRadius: 14, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
        <Text style={{ fontFamily: t.font.display, fontSize: 15, color: waiting ? t.accInk : t.mut }}>?</Text>
      </View>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: waiting ? t.accInk : t.mut }}>{tr('said.guess.who')}</Text>
    </Animated.View>
  );
}

/** Who I said wrote it, signed at the bottom of the card. On the selected card, an × takes them off. */
function Signature({ member, selected, onClear }: { member: Member; selected: boolean; onClear: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View key={member.id} entering={motion.pop} style={{ flexDirection: 'row', alignItems: 'center', gap: 6, flexShrink: 1 }}>
      <View
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          gap: 8,
          height: 36,
          paddingLeft: 4,
          paddingRight: 14,
          borderRadius: t.radius.pill,
          backgroundColor: selected ? t.card : t.card2,
          flexShrink: 1,
        }}>
        <Avatar member={member} size={28} />
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
          {firstName(member.name)}
        </Text>
      </View>
      {selected && (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('said.guess.clear', { name: member.name })}
          onPress={onClear}
          hitSlop={8}
          style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
          <X size={16} color={t.mut} />
        </Pressable>
      )}
    </Animated.View>
  );
}
