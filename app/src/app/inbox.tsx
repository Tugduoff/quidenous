import { router } from 'expo-router';
import { AtSign, Bell, BellRing, CalendarHeart, ChevronLeft, Crown, DoorOpen, KeyRound, type LucideIcon, Medal, MessageCircle, MessageCircleQuestion, Sparkles, UserPlus } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { type TKey, useT } from '@/lib/i18n';
import { useSwitchRoom, useSwitchRoomHere } from '@/lib/hooks/use-switch-room';
import { useSession } from '@/lib/session';
import { useMarkInboxRead } from '@/lib/hooks/use-inbox';
import { tidy } from '@/lib/utils/text';
import { useTheme } from '@/lib/theme';
import { ago } from '@/lib/utils/time';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { Display } from '@/components/ui/text';
import { type InboxItem, useInbox } from '@/lib/api/inbox';
import { useGroup, useMembers } from '@/lib/api/group';

// A kind this version doesn't know (newer server) falls back to a plain bell and title.
const ICON: Record<string, LucideIcon> = {
  message: MessageCircle,
  mention: AtSign,
  nudge: BellRing,
  result: Crown,
  question: Sparkles,
  added: UserPlus,
  ask: MessageCircleQuestion,
  friend_request: UserPlus,
  invite: DoorOpen,
  owner: KeyRound,
  badge: Medal,
  wrapped: CalendarHeart,
};
const TITLE: Record<string, TKey> = {
  message: 'inbox.kinds.message',
  mention: 'inbox.kinds.mention',
  nudge: 'inbox.kinds.nudge',
  result: 'inbox.kinds.result',
  question: 'inbox.kinds.question',
  added: 'inbox.kinds.added',
  ask: 'inbox.kinds.ask',
  friend_request: 'inbox.kinds.friendRequest',
  invite: 'inbox.kinds.invite',
  owner: 'inbox.kinds.owner',
  badge: 'inbox.kinds.badge',
  wrapped: 'inbox.kinds.wrapped',
};

/** Everything that happened lately: results, new questions, nudges, messages. Opening it marks all as read. */
export default function InboxScreen() {
  const t = useTheme();
  const tr = useT();
  const inbox = useInbox();
  const { data } = inbox;
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
  if (data && !fresh) setFresh(new Set(data.items.filter((i) => !i.read).map((i) => i.id)));
  useMarkInboxRead(unread);

  const open = (item: InboxItem) => {
    // Not about this room: a friend request (Friends) or an invitation into another room (my rooms).
    if (item.kind === 'friend_request') return router.push('/friends');
    if (item.kind === 'invite') return router.push('/rooms');
    // About me, whatever the room: my collection, my year.
    if (item.kind === 'badge') return router.push('/badges');
    if (item.kind === 'wrapped') return router.push('/wrapped');
    // Added to a room, or now its owner: go to it (switching rooms lands on its home screen).
    if (item.kind === 'added' || item.kind === 'owner') {
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
        accessibilityLabel={tr('common.back')}
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
        hitSlop={4}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>{tr('inbox.title')}</Display>
      <QueryState query={inbox} />
      {data && data.items.length === 0 && <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>{tr('inbox.empty')}</Text>}
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
              ? tr('inbox.mentioned', { name: actor.name })
              : tr(TITLE[item.kind] ?? 'inbox.kinds.other');
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
                  {tidy(item.text)}
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
