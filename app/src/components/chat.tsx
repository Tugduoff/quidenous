import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Send } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { Avatar, Card } from '@/components/ui';
import { api, type Message } from '@/lib/api';
import { keys, useMembers, useMessages } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

const TIME = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' });
const MAX = 500;

/** The conversation under a result: everyone can react once votes are out. */
export function Chat({ roundId }: { roundId: number }) {
  const t = useTheme();
  const qc = useQueryClient();
  const { me } = useSession();
  const { byId } = useMembers();
  const { data: messages } = useMessages(roundId);
  const [text, setText] = useState('');
  const send = useMutation({
    mutationFn: (body: string) => api<Message>(`/rounds/${roundId}/messages`, { method: 'POST', json: { text: body } }),
    onSuccess: (m) => {
      setText('');
      qc.setQueryData<Message[]>(keys.messages(roundId), (old) => (old?.some((x) => x.id === m.id) ? old : [...(old ?? []), m]));
    },
  });
  const ok = text.trim().length > 0 && !send.isPending;
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>
        {messages?.length ? `${messages.length} message${messages.length > 1 ? 's' : ''}` : 'Réagir'}
      </Text>
      {messages?.map((m) => {
        const author = byId.get(m.member_id);
        const mine = m.member_id === me?.id;
        return (
          <View key={m.id} style={{ flexDirection: mine ? 'row-reverse' : 'row', gap: 8, alignItems: 'flex-end' }}>
            {!mine && <Avatar member={author} size={28} />}
            <View
              style={{
                maxWidth: '78%',
                paddingHorizontal: 14,
                paddingVertical: 9,
                borderRadius: 18,
                borderBottomLeftRadius: mine ? 18 : 6,
                borderBottomRightRadius: mine ? 6 : 18,
                backgroundColor: mine ? t.acc : t.card,
                gap: 2,
              }}>
              {!mine && <Text style={{ fontFamily: t.font.bold, fontSize: 12, color: author?.color ?? t.mut }}>{author?.name ?? '?'}</Text>}
              <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 20, color: mine ? t.accTxt : t.txt }}>{m.text}</Text>
              <Text style={{ fontFamily: t.font.regular, fontSize: 10, color: mine ? t.accTxt : t.mut, opacity: 0.7, alignSelf: 'flex-end' }}>
                {TIME.format(new Date(m.created_at))}
              </Text>
            </View>
          </View>
        );
      })}
      <Card style={{ flexDirection: 'row', alignItems: 'flex-end', gap: 8, padding: 6, paddingLeft: 16 }}>
        <TextInput
          accessibilityLabel="Écrire un message"
          placeholder="Un avis sur ce résultat ?"
          placeholderTextColor={t.mut}
          value={text}
          onChangeText={setText}
          multiline
          maxLength={MAX}
          style={{ flex: 1, minHeight: 40, maxHeight: 120, paddingVertical: 10, fontFamily: t.font.regular, fontSize: 15, color: t.txt }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Envoyer"
          disabled={!ok}
          onPress={() => send.mutate(text.trim())}
          style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: ok ? t.acc : t.card2 }}>
          <Send size={18} color={ok ? t.accTxt : t.mut} />
        </Pressable>
      </Card>
    </View>
  );
}
