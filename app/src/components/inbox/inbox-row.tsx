import { AtSign, Bell, BellRing, CalendarHeart, Crown, DoorOpen, KeyRound, type LucideIcon, Medal, MessageCircle, MessageCircleQuestion, Sparkles, UserPlus } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import type { InboxItem } from '@/lib/api/inbox';
import { type TKey, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { tidy } from '@/lib/utils/text';
import { ago } from '@/lib/utils/time';

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

export function InboxRow({ item, actor, isNew, onOpen }: { item: InboxItem; actor: Member | undefined; isNew: boolean; onOpen: () => void }) {
  const t = useTheme();
  const tr = useT();
  const Icon = ICON[item.kind] ?? Bell;
  const head =
    item.kind === 'message' && actor
      ? actor.name
      : item.kind === 'mention' && actor
        ? tr('inbox.mentioned', { name: actor.name })
        : tr(TITLE[item.kind] ?? 'inbox.kinds.other');
  return (
    <Pressable accessibilityRole="button" onPress={onOpen}>
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
}
