import { Text, View } from 'react-native';
import { AnonymousMark } from '@/components/vote/anonymous';
import type { PackQuestion } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { QuestionText } from '@/components/ui/text';

export function PackQuestionPreview({ questions }: { questions: PackQuestion[] }) {
  const t = useTheme();
  return (
  <Card style={{ padding: 16, gap: 10 }}>
    {questions.map((q, i) => (
      <View key={i} style={{ gap: 2 }}>
        <View style={{ flexDirection: 'row', alignItems: 'flex-start', gap: 6 }}>
          <View style={{ flex: 1 }}>
            <QuestionText
              text={q.text}
              rank={q.kind === 'rank'}
              choose={q.kind === 'choose'}
              style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 20, color: t.txt }}
            />
          </View>
          {/* Anonymous votes once installed. */}
          {q.anonymous && (
            <View style={{ marginTop: 3 }}>
              <AnonymousMark />
            </View>
          )}
        </View>
        {/* A choose question's items, in one line under it. */}
        {q.kind === 'choose' && !!q.items?.length && (
          <Text numberOfLines={1} style={{ marginLeft: 19, fontFamily: t.font.regular, fontSize: 12, lineHeight: 17, color: t.mut }}>
            {q.items.join(' · ')}
          </Text>
        )}
      </View>
    ))}
  </Card>
  );
}
