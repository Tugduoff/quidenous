import { Cable, Check, Trophy } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';
import { AnonymousMark } from '@/components/vote/anonymous';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import type { DraftLine } from '@/lib/utils/import-draft';
import { tidy } from '@/lib/utils/text';

export function DraftLineRow({ line: l, anonymousVotes, onToggle }: { line: DraftLine; anonymousVotes: boolean; onToggle: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Pressable
      accessibilityRole="checkbox"
      accessibilityState={{
        checked: l.selected,
        disabled: !!l.problem,
      }}
      onPress={onToggle}
      style={{
        flexDirection: 'row',
        gap: 10,
        alignItems: 'flex-start',
        opacity: l.problem ? 0.5 : 1,
      }}>
      <View
        style={{
          width: 22,
          height: 22,
          borderRadius: 7,
          marginTop: 1,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: l.selected ? t.acc : 'transparent',
          borderWidth: l.selected ? 0 : 2,
          borderColor: t.line,
        }}>
        {l.selected && <Check size={14} color={t.accTxt} strokeWidth={3} />}
      </View>
      {l.rank && <Trophy size={14} color={t.accInk} style={{ marginTop: 3 }} />}
      {l.kind === 'choose' && <Cable size={14} color={t.accInk} style={{ marginTop: 3 }} />}
      {l.anonymous && anonymousVotes && (
        <View style={{ marginTop: 3 }}>
          <AnonymousMark />
        </View>
      )}
      <View style={{ flex: 1 }}>
        <Text
          style={{
            fontFamily: t.font.regular,
            fontSize: 14,
            lineHeight: 20,
            color: t.txt,
          }}>
          {tidy(l.text)}
          {l.problem && <Text style={{ color: t.tagInk }}>{`  · ${tr(`questions.problem.${l.problem}`)}`}</Text>}
        </Text>
        {!!l.items?.length && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
            {l.items.join(' · ')}
          </Text>
        )}
      </View>
    </Pressable>
  );
}
