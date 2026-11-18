import { CircleCheck, CircleX, Hourglass, ThumbsUp } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import type { Suggestion } from '@/lib/api/community';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { QuestionText } from '@/components/ui/text';

/** My suggestions and where each one stands; the three newest unless opened. */
export function MySuggestions({ items }: { items: Suggestion[] }) {
  const t = useTheme();
  const tr = useT();
  const [all, setAll] = useState(false);
  if (!items.length) return null;
  const shown = all ? items : items.slice(0, 3);
  return (
    <View style={{ gap: 10, marginTop: 6 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{tr('community.mine.title')}</Text>
        {items.length > 3 && (
          <Pressable accessibilityRole="button" onPress={() => setAll(!all)} hitSlop={8}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>
              {tr(all ? 'community.mine.less' : 'community.mine.more')}
            </Text>
          </Pressable>
        )}
      </View>
      {shown.map((s) => (
        <Animated.View key={s.id} entering={motion.enter} layout={motion.layout}>
          <Card style={{ padding: 14, gap: 6 }}>
            <QuestionText
              text={s.text}
              rank={s.kind === 'rank'}
              style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}
            />
            <Status s={s} />
          </Card>
        </Animated.View>
      ))}
    </View>
  );
}

function Status({ s }: { s: Suggestion }) {
  const t = useTheme();
  const tr = useT();
  const score = s.up - s.down;
  const [Icon, text, color] =
    s.status === 'approved'
      ? [CircleCheck, tr('community.mine.approved'), t.accInk]
      : s.status === 'rejected'
        ? [CircleX, tr('community.mine.rejected'), t.mut]
        : s.status === 'queued'
          ? [Hourglass, tr('community.mine.queued'), t.mut]
          : [ThumbsUp, tr('community.mine.voting', { score: score > 0 ? `+${score}` : String(score) }), t.mut];
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
      <Icon size={14} color={color} />
      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 13, color }}>
        {text}
      </Text>
    </View>
  );
}
