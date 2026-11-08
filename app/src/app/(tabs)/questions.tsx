import { useMutation, useQueryClient } from '@tanstack/react-query';
import { MessageCircleQuestion, Plus } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, Card, Display } from '@/components/ui';
import { api } from '@/lib/api';
import { keys, useMembers, useQueue } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const MIN = 8;

export default function QuestionsScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data } = useQueue();
  const { byId } = useMembers();
  const [text, setText] = useState('');
  const add = useMutation({
    mutationFn: () => api('/questions', { method: 'POST', json: { text } }),
    onSuccess: () => {
      setText('');
      qc.invalidateQueries({ queryKey: keys.queue });
      qc.invalidateQueries({ queryKey: keys.current });
    },
  });
  const ok = text.trim().length >= MIN && !add.isPending;
  return (
    <Screen>
      <Display size={30}>Questions</Display>
      <View style={{ flexDirection: 'row', gap: 10, alignItems: 'flex-end' }}>
        <Card style={{ flex: 1 }}>
          <TextInput
            accessibilityLabel="Nouvelle question"
            placeholder="Qui de nous…"
            placeholderTextColor={t.mut}
            value={text}
            onChangeText={setText}
            multiline
            maxLength={200}
            style={{ minHeight: 96, padding: 16, fontFamily: t.font.regular, fontSize: 16, color: t.txt, textAlignVertical: 'top' }}
          />
        </Card>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Ajouter"
          disabled={!ok}
          onPress={() => add.mutate()}
          style={{ width: 52, height: 52, borderRadius: 26, backgroundColor: ok ? t.acc : t.card2, alignItems: 'center', justifyContent: 'center' }}>
          <Plus size={26} color={ok ? t.accTxt : t.mut} />
        </Pressable>
      </View>
      <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
        {(data?.custom.length ?? 0) + (data?.deck_left ?? 0)}
      </Text>
      {data?.custom.map((q) => (
        <Card key={q.id} style={{ flexDirection: 'row', gap: 12, alignItems: 'center', padding: 14 }}>
          {q.author_id != null ? (
            <Avatar member={byId.get(q.author_id)} size={32} />
          ) : (
            <MessageCircleQuestion size={20} color={t.mut} />
          )}
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{q.text}</Text>
        </Card>
      ))}
    </Screen>
  );
}
