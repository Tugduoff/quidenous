import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Send } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Pressable, ScrollView, Text, TextInput, View } from 'react-native';

import { ModerationDialog, type ModTarget } from '@/components/moderation';
import { Avatar, Card } from '@/components/ui';
import { api, type Message } from '@/lib/api';
import { useBlocks } from '@/lib/moderation';
import { keys, useMembers, useMessages } from '@/lib/queries';
import { sendOnEnter } from '@/lib/submit';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

const TIME = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' });
const MAX = 500;

/** The conversation under a result, in its own scrollable box that follows the newest message. */
export function ChatMessages({ roundId }: { roundId: number }) {
  const t = useTheme();
  const { me } = useSession();
  const { byId } = useMembers();
  const { data } = useMessages(roundId);
  const { ids: blocked } = useBlocks();
  const box = useRef<ComponentRef<typeof ScrollView>>(null);
  // Long-press on someone else's message: report it, block its author.
  const [target, setTarget] = useState<ModTarget | null>(null);
  const authorId = (m: Message) => m.user_id ?? byId.get(m.member_id)?.user_id;
  // People I blocked: gone at once, live messages included (the server leaves them out after a reload).
  const messages = data?.filter((m) => !blocked.has(authorId(m) ?? -1));
  // No messages yet: nothing to show, the box below already invites one.
  if (!messages?.length) return null;
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>
        {`${messages.length} message${messages.length > 1 ? 's' : ''}`}
      </Text>
      <ScrollView
        ref={box}
        nestedScrollEnabled
        style={{ maxHeight: 320, borderRadius: 20 }}
        contentContainerStyle={{ gap: 10 }}
        onContentSizeChange={() => box.current?.scrollToEnd({ animated: false })}>
        {messages.map((m) => {
          const author = byId.get(m.member_id);
          const mine = m.member_id === me?.id;
          const userId = authorId(m);
          const onLongPress = mine
            ? undefined
            : () =>
                setTarget({
                  kind: 'message',
                  id: m.id,
                  person: author && userId != null ? { user_id: userId, name: author.name, color: author.color, photo: author.photo_url } : undefined,
                });
          return (
            <View key={m.id} style={{ flexDirection: mine ? 'row-reverse' : 'row', gap: 8, alignItems: 'flex-end' }}>
              {!mine && <Avatar member={author} size={28} />}
              <Pressable
                onLongPress={onLongPress}
                delayLongPress={400}
                accessibilityHint={mine ? undefined : 'Appui long : signaler ou bloquer'}
                style={{
                  maxWidth: '78%',
                  paddingHorizontal: 14,
                  paddingVertical: 9,
                  borderRadius: 18,
                  borderBottomLeftRadius: mine ? 18 : 6,
                  borderBottomRightRadius: mine ? 6 : 18,
                  // My own bubbles in my colour: lime is for actions (Suivante, Envoyer).
                  backgroundColor: mine ? (me?.color ?? t.card2) : t.card,
                  gap: 2,
                }}>
                {!mine && <Text style={{ fontFamily: t.font.bold, fontSize: 12, color: author?.color ?? t.mut }}>{author?.name ?? '?'}</Text>}
                <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 20, color: mine ? '#121216' : t.txt }}>{m.text}</Text>
                <Text style={{ fontFamily: t.font.regular, fontSize: 10, color: mine ? '#121216' : t.mut, opacity: 0.7, alignSelf: 'flex-end' }}>
                  {TIME.format(new Date(m.created_at))}
                </Text>
              </Pressable>
            </View>
          );
        })}
      </ScrollView>
      <ModerationDialog target={target} onClose={() => setTarget(null)} />
    </View>
  );
}

/** The message box, meant for the screen footer (always reachable, above Suivante, rises with the keyboard). */
export function ChatInput({ roundId }: { roundId: number }) {
  const t = useTheme();
  const qc = useQueryClient();
  const [text, setText] = useState('');
  const send = useMutation({
    mutationFn: (body: string) => api<Message>(`/rounds/${roundId}/messages`, { method: 'POST', json: { text: body } }),
    onSuccess: (m) => {
      setText('');
      qc.setQueryData<Message[]>(keys.messages(roundId), (old) => (old?.some((x) => x.id === m.id) ? old : [...(old ?? []), m]));
    },
  });
  const ok = text.trim().length > 0 && !send.isPending;
  const submit = () => ok && send.mutate(text.trim());
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'flex-end', gap: 8, padding: 6, paddingLeft: 16 }}>
      <TextInput
        accessibilityLabel="Écrire un message"
        placeholder="Un avis sur ce résultat ?"
        placeholderTextColor={t.mut}
        value={text}
        onChangeText={setText}
        multiline
        maxLength={MAX}
        {...sendOnEnter(submit)}
        // Under 16px, Safari on iPhone zooms the page in when the field gets the focus.
        style={{ flex: 1, minHeight: 40, maxHeight: 100, paddingVertical: 10, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
      />
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Envoyer"
        disabled={!ok}
        onPress={submit}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: ok ? t.acc : t.card2 }}>
        <Send size={18} color={ok ? t.accTxt : t.mut} />
      </Pressable>
    </Card>
  );
}
