import { memo, useCallback } from 'react';
import { ScrollView, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { NumberDisc } from '@/components/said/number-disc';
import { Avatar } from '@/components/ui/avatar';
import { PressableScale } from '@/components/ui/pressable-scale';
import type { Member } from '@/lib/api/group';
import { impactHaptic } from '@/lib/device/haptics';
import { useT } from '@/lib/i18n';
import { usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { firstName } from '@/lib/utils/names';
import { isComplete, type Match, type MatchAction } from '@/lib/utils/said';

/**
 * The people to place, pinned over the button: tap one to put them on the waiting answer. Someone already placed shows
 * the answer's number (tapping them moves them, or swaps with whoever was there).
 */
export function SuspectTray({ people, state, dispatch }: { people: Member[]; state: Match; dispatch: (a: MatchAction) => void }) {
  const t = useTheme();
  const tr = useT();
  const onPick = useCallback(
    (id: number) => {
      impactHaptic();
      dispatch({ type: 'pick', member: id });
    },
    [dispatch],
  );
  const slotOf = new Map<number, number>();
  state.order.forEach((id, i) => {
    const m = state.picks[id];
    if (m != null) slotOf.set(m, i + 1);
  });
  const waiting = state.sel != null ? state.order.indexOf(state.sel) + 1 : 0;
  const chips = people.map((m) => <SuspectChip key={m.id} member={m} slot={slotOf.get(m.id) ?? 0} onPick={onPick} />);
  // A handful wraps on two lines at most; a crowd scrolls sideways on one.
  const crowd = people.length > 6;
  return (
    <View style={{ gap: 8 }}>
      <Text
        accessibilityLiveRegion="polite"
        numberOfLines={1}
        style={{ textAlign: 'center', fontFamily: t.font.semibold, fontSize: 13, color: waiting ? t.txt : t.mut }}>
        {waiting ? tr('said.guess.whoWrote', { n: waiting }) : isComplete(state) ? tr('said.guess.allSet') : tr('said.guess.tapAnswer')}
      </Text>
      {crowd ? (
        <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={{ gap: 8, paddingHorizontal: 2, paddingVertical: 2 }}>
          {chips}
        </ScrollView>
      ) : (
        <View style={{ flexDirection: 'row', flexWrap: 'wrap', justifyContent: 'center', gap: 8 }}>{chips}</View>
      )}
    </View>
  );
}

const SuspectChip = memo(function SuspectChip({
  member,
  slot,
  onPick,
}: {
  member: Member;
  /** The answer they're on (its number), 0: none. */
  slot: number;
  onPick: (id: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const used = slot > 0;
  const bump = usePop(slot, 1.08, used);
  return (
    <Animated.View style={bump}>
      <PressableScale
        scale={0.94}
        accessibilityRole="button"
        accessibilityLabel={used ? tr('said.guess.placed', { name: member.name, n: slot }) : tr('said.guess.assign', { name: member.name })}
        onPress={() => onPick(member.id)}
        style={[
          {
            flexDirection: 'row',
            alignItems: 'center',
            gap: 8,
            height: 48,
            paddingLeft: 6,
            paddingRight: used ? 8 : 16,
            borderRadius: t.radius.pill,
            backgroundColor: used ? t.card2 : t.card,
          },
          !used && t.shadow,
        ]}>
        <View style={{ opacity: used ? 0.55 : 1 }}>
          <Avatar member={member} size={34} />
        </View>
        <Text numberOfLines={1} style={{ maxWidth: 110, fontFamily: t.font.semibold, fontSize: 15, color: used ? t.mut : t.txt }}>
          {firstName(member.name)}
        </Text>
        {used && <NumberDisc n={slot} />}
      </PressableScale>
    </Animated.View>
  );
});
