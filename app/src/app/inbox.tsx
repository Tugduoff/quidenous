import { useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { AtSign, Bell, BellRing, ChevronLeft, Crown, type LucideIcon, MessageCircle, MessageCircleQuestion, Sparkles, UserPlus } from 'lucide-react-native';
import { useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, Card, Display } from '@/components/ui';
import { api, type InboxItem } from '@/lib/api';
import { keys, useGroup, useInbox, useMembers } from '@/lib/queries';
import { useSwitchRoom, useSwitchRoomHere } from '@/lib/rooms';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { ago } from '@/lib/time';

// A kind this version doesn't know (newer server) falls back to a plain bell and title.
const ICON: Record<string, LucideIcon> = {
  message: MessageCircle,
  mention: AtSign,
  nudge: BellRing,
  result: Crown,
  question: Sparkles,
  added: UserPlus,
  ask: MessageCircleQuestion,
};
const TITLE: Record<string, string> = {
  message: 'Nouveau message',
  mention: 'Mention',
  nudge: 'Relance',
  result: 'Résultat dispo',
  question: 'Nouvelle question',
  added: 'Ajouté à un groupe',
  ask: 'Questions',
};

/** Everything that happened lately: results, new questions, nudges, messages. Opening it marks all as read. */
export default function InboxScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data } = useInbox();
  const { byId } = useMembers();
  // The room on screen: the session's, or the one the server picked (session still null).
  const { data: group } = useGroup();
  const session = useSession();
  const room = group?.id ?? session.room;
  const switchRoom = useSwitchRoom();
  const switchHere = useSwitchRoomHere();
  const unread = data?.unread ?? 0;
  // What was new when I opened the screen stays highlighted while I look, even once marked as read.
  const [fresh, setFresh] = useState<Set<number> | null>(null);
  useEffect(() => {
    if (data && !fresh) setFresh(new Set(data.items.filter((i) => !i.read).map((i) => i.id)));
  }, [data, fresh]);
  useEffect(() => {
    if (unread > 0) api('/inbox/read', { method: 'POST' }).then(() => qc.invalidateQueries({ queryKey: keys.inbox }));
  }, [unread, qc]);

  const open = (item: InboxItem) => {
    // Added to a room: go to it (switching rooms lands on its home screen).
    if (item.kind === 'added') {
      if (item.room_id != null && item.room_id !== room) switchRoom(item.room_id);
      else router.replace('/');
      return;
    }
    // Someone asks me (the owner) for questions: the Questions tab, in that room.
    if (item.kind === 'ask') {
      if (item.room_id != null && item.room_id !== room)
        switchHere(item.room_id)
          .catch(() => false)
          .then((ok) => router.replace(ok ? '/questions' : '/'));
      else router.replace('/questions');
      return;
    }
    // A message or mention under a result of another room: open it in that room (one I left: home instead).
    if ((item.kind === 'mention' || item.kind === 'message') && item.round_id != null && item.room_id != null && item.room_id !== room) {
      const id = item.round_id;
      switchHere(item.room_id)
        .catch(() => false)
        .then((ok) => (ok ? router.push(`/round/${id}`) : router.replace('/')));
      return;
    }
    if (item.kind === 'nudge' || item.kind === 'question' || item.round_id == null) router.replace('/');
    else router.push(`/round/${item.round_id}`);
  };

  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>Notifications</Display>
      {data && data.items.length === 0 && (
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>Rien de neuf.</Text>
      )}
      {data?.items.map((item) => {
        const Icon = ICON[item.kind] ?? Bell;
        const isNew = !item.read || !!fresh?.has(item.id);
        // Member ids belong to one room: only look the actor up among the people of the room on screen.
        const here = item.room_id == null || item.room_id === room;
        const actor = item.actor_id != null && here ? byId.get(item.actor_id) : undefined;
        const head =
          item.kind === 'message' && actor
            ? actor.name
            : item.kind === 'mention' && actor
              ? `${actor.name} t'a mentionné`
              : (TITLE[item.kind] ?? 'Notification');
        return (
          <Pressable key={item.id} accessibilityRole="button" onPress={() => open(item)}>
            <Card style={{ flexDirection: 'row', gap: 12, alignItems: 'center', padding: 14 }}>
              {actor ? (
                <Avatar member={actor} size={40} />
              ) : (
                <View style={{ width: 40, height: 40, borderRadius: 20, backgroundColor: t.accSoft, alignItems: 'center', justifyContent: 'center' }}>
                  <Icon size={20} color={t.accInk} />
                </View>
              )}
              <View style={{ flex: 1, gap: 2 }}>
                <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 13, color: t.mut }}>
                  {head} · {ago(item.created_at)}
                </Text>
                <Text numberOfLines={2} style={{ fontFamily: isNew ? t.font.semibold : t.font.regular, fontSize: 15, lineHeight: 20, color: t.txt }}>
                  {item.text}
                </Text>
              </View>
              {isNew && <View style={{ width: 10, height: 10, borderRadius: 5, backgroundColor: t.tag }} />}
            </Card>
          </Pressable>
        );
      })}
    </Screen>
  );
}
