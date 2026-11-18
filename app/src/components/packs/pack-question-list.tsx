import { Cable, Trophy, X } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { AnonymousMark } from '@/components/vote/anonymous';
import { type Selection, SelectableRow } from '@/components/ui/selection';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import type { PackQuestion } from '@/lib/api/packs';
import { tidy } from '@/lib/utils/text';
import { questionKey as keyOf } from '@/lib/utils/pack-draft';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';

export function PackQuestionList({
  questions,
  setQuestions,
  sel,
  anonymousVotes,
}: {
  questions: PackQuestion[];
  setQuestions: (update: (qs: PackQuestion[]) => PackQuestion[]) => void;
  sel: Selection<string>;
  anonymousVotes: boolean;
}) {
  const t = useTheme();
  const tr = useT();
  if (!questions.length) return null;
  return (
    <Card style={{ paddingVertical: 4 }}>
      {questions.map((q, i) => {
        const rank = q.kind === 'rank';
        const choose = q.kind === 'choose';
        return (
          <Animated.View
            key={keyOf(q.text)}
            entering={motion.enter}
            exiting={motion.exit}
            layout={motion.layout}
            // Selecting: the checkbox in line with the add field's text, not against the card's edge.
            style={{ paddingLeft: sel.on ? 14 : 0, borderTopWidth: i ? 1 : 0, borderColor: t.line }}>
            <SelectableRow sel={sel} id={keyOf(q.text)} label={tidy(q.text)}>
              {/* The hold selects; the toggle and the cross inside keep their own taps. */}
              <Pressable
                accessible={false}
                onLongPress={() => sel.start(keyOf(q.text))}
                delayLongPress={400}
                style={{ flexDirection: 'row', alignItems: 'center', gap: 6, paddingLeft: 6, paddingRight: 6 }}>
                {/* A choose question stays one; the others switch between pick and ranking. */}
                {choose ? (
                  <View
                    accessibilityLabel={tr('vote.kind.choose')}
                    style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.accSoft }}>
                    <Cable size={16} color={t.accInk} />
                  </View>
                ) : (
                  <Pressable
                    accessibilityRole="switch"
                    accessibilityLabel={tr('vote.kind.rank')}
                    accessibilityState={{ checked: rank }}
                    hitSlop={4}
                    onPress={() => setQuestions((qs) => qs.map((x, j) => (j === i ? { ...x, kind: rank ? 'pick' : 'rank' } : x)))}
                    style={{
                      width: 36,
                      height: 36,
                      borderRadius: 18,
                      alignItems: 'center',
                      justifyContent: 'center',
                      backgroundColor: rank ? t.accSoft : 'transparent',
                    }}>
                    <Trophy size={16} color={rank ? t.accInk : t.mut} style={{ opacity: rank ? 1 : 0.5 }} />
                  </Pressable>
                )}
                <View style={{ flex: 1, paddingVertical: 10, gap: 2 }}>
                  <Text style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 20, color: t.txt }}>{tidy(q.text)}</Text>
                  {choose && !!q.items?.length && (
                    <Text style={{ fontFamily: t.font.regular, fontSize: 12, lineHeight: 17, color: t.mut }}>{q.items.join(' · ')}</Text>
                  )}
                </View>
                {anonymousVotes && q.anonymous && !choose && <AnonymousMark />}
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel={tr('packs.removeQuestion')}
                  hitSlop={4}
                  onPress={() => setQuestions((qs) => qs.filter((_, j) => j !== i))}
                  style={{ width: 36, height: 36, alignItems: 'center', justifyContent: 'center' }}>
                  <X size={16} color={t.mut} />
                </Pressable>
              </Pressable>
            </SelectableRow>
          </Animated.View>
        );
      })}
    </Card>
  );
}
