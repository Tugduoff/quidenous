import { Users } from 'lucide-react-native';
import { Pressable, ScrollView, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Avatar } from '@/components/ui/avatar';
import type { Member } from '@/lib/api/group';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { everyone } from '@/lib/utils/mentions';

export type Suggestion = { key: string; insert: string; member?: Member };

/** One suggestion row; the list shows about five and a half, so a longer one visibly scrolls. */
const ROW = 40;

export function MentionList({
  rows,
  active,
  onPick,
  onHover,
}: {
  rows: Suggestion[];
  active: number;
  onPick: (row: Suggestion) => void;
  onHover: (i: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View entering={motion.rise} style={{ borderBottomWidth: 1, borderColor: t.card2, paddingBottom: 4 }}>
      <ScrollView style={{ maxHeight: ROW * 5.5 }} keyboardShouldPersistTaps="handled" nestedScrollEnabled>
        {rows.map((r, i) => (
          <Pressable
            key={r.key}
            accessibilityRole="button"
            accessibilityLabel={r.member ? tr('chat.mention', { name: r.member.name }) : tr('chat.mentionAll')}
            onPress={() => onPick(r)}
            onHoverIn={() => onHover(i)}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 10,
              height: ROW,
              paddingHorizontal: 8,
              borderRadius: 14,
              backgroundColor: i === active ? t.card2 : undefined,
            }}>
            {r.member ? (
              <Avatar member={r.member} size={28} />
            ) : (
              <View
                style={{ width: 28, height: 28, borderRadius: 14, backgroundColor: t.accSoft, alignItems: 'center', justifyContent: 'center' }}>
                <Users size={15} color={t.accInk} />
              </View>
            )}
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
              {r.member ? r.member.name : everyone()}
            </Text>
            {!r.member && <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('chat.everyoneRow')}</Text>}
          </Pressable>
        ))}
      </ScrollView>
    </Animated.View>
  );
}
