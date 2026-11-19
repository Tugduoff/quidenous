import { ArrowUp, Trophy } from 'lucide-react-native';
import { useState } from 'react';
import { Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { communityError, SUGGESTION_MAX, SUGGESTION_MIN, useSuggest } from '@/lib/api/community';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { sendOnEnter } from '@/components/ui/send-on-enter';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';
import { Toggle } from '@/components/ui/toggle';

export function SuggestCompose({ left }: { left: number | undefined }) {
  const t = useTheme();
  const tr = useT();
  const [text, setText] = useState('');
  const [rank, setRank] = useState(false);
  const [note, setNote] = useState<{ text: string; error: boolean } | null>(null);
  const suggest = useSuggest();
  const out = left === 0;
  const ok = text.trim().length >= SUGGESTION_MIN && !out && !suggest.isPending;
  const send = () => {
    if (!ok) return;
    setNote(null);
    suggest.mutate(
      { text: text.trim(), kind: rank ? 'rank' : 'pick' },
      {
        onSuccess: () => {
          setText('');
          setRank(false);
          setNote({ text: tr('community.compose.sent'), error: false });
        },
        onError: (e) => setNote({ text: communityError(e), error: true }),
      },
    );
  };
  return (
    <View style={{ gap: 8 }}>
      <Card style={{ paddingBottom: 10 }}>
        <TextInput
          accessibilityLabel={tr('community.compose.label')}
          placeholder={rank ? tr('community.compose.placeholderRank') : tr('community.compose.placeholder')}
          placeholderTextColor={t.mut}
          value={text}
          onChangeText={(v) => {
            setText(v);
            if (note) setNote(null);
          }}
          multiline
          maxLength={SUGGESTION_MAX}
          {...sendOnEnter(send)}
          style={{ minHeight: 76, padding: 16, paddingBottom: 6, fontFamily: t.font.regular, fontSize: 16, color: t.txt, textAlignVertical: 'top' }}
        />
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingLeft: 16, paddingRight: 10 }}>
          <Toggle icon={Trophy} label={tr('vote.kind.rank')} value={rank} onChange={setRank} />
          <View style={{ flex: 1 }} />
          {left != null && (
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 13, color: out ? t.tagInk : t.mut }}>
              {tr('community.compose.left', { count: left })}
            </Text>
          )}
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('community.compose.send')}
            disabled={!ok}
            onPress={send}
            style={{
              width: 44,
              height: 44,
              borderRadius: 22,
              backgroundColor: ok ? t.acc : t.card2,
              alignItems: 'center',
              justifyContent: 'center',
            }}>
            <ArrowUp size={22} color={ok ? t.accTxt : t.mut} />
          </PressableScale>
        </View>
      </Card>
      {note && (
        <Animated.Text
          entering={motion.enter}
          style={{ marginLeft: 4, fontFamily: t.font.semibold, fontSize: 14, color: note.error ? t.tagInk : t.accInk }}>
          {note.text}
        </Animated.Text>
      )}
    </View>
  );
}
