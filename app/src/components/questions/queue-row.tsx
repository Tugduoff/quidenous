import { MessageCircleQuestion, Trash2 } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';
import { AnonymousMark } from '@/components/vote/anonymous';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { QuestionText } from '@/components/ui/text';
import type { Member } from '@/lib/api/group';
import type { QueuedQuestion } from '@/lib/api/questions';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function QueueRow({
  q,
  member,
  perQuestion,
  canDelete,
  onDelete,
}: {
  q: QueuedQuestion;
  member: Member | undefined;
  perQuestion: boolean;
  canDelete: boolean;
  onDelete: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  return (
    <Card
      style={{
        flexDirection: 'row',
        gap: 12,
        alignItems: 'center',
        padding: 14,
      }}>
      {q.author_id != null ? <Avatar member={member} size={32} /> : <MessageCircleQuestion size={20} color={t.mut} />}
      <View style={{ flex: 1, gap: 2 }}>
        <QuestionText
          text={q.text}
          rank={q.kind === 'rank'}
          choose={q.kind === 'choose'}
          said={q.kind === 'said'}
          style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}
        />
        {!!q.items?.length && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
            {q.items.join(' · ')}
          </Text>
        )}
      </View>
      {perQuestion && q.anonymous && <AnonymousMark />}
      {canDelete && (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('questions.deleteQuestion')}
          onPress={onDelete}
          hitSlop={8}
          style={{
            width: 34,
            height: 34,
            borderRadius: 17,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: t.card2,
          }}>
          <Trash2 size={16} color={t.mut} />
        </Pressable>
      )}
    </Card>
  );
}
