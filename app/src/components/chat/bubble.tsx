import { EyeOff } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Platform, Pressable, Text, type TextStyle, View, type ViewStyle } from 'react-native';
import Animated from 'react-native-reanimated';
import { MessageMediaFrame } from '@/components/chat/media';
import { HeartBurst, ReactionChips } from '@/components/chat/reactions';
import type { ModTarget } from '@/components/moderation/moderation';
import { Avatar } from '@/components/ui/avatar';
import type { Message } from '@/lib/api/chat';
import { type Member, useMembers } from '@/lib/api/group';
import { impactHaptic } from '@/lib/device/haptics';
import { useUnmountEffect } from '@/lib/hooks/use-mount-effect';
import { dateFormat, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { HEART, mediaOf } from '@/lib/utils/chat';
import { segments } from '@/lib/utils/mentions';

const soft = (color: string) => (/^#[0-9a-f]{6}$/i.test(color) ? `${color}33` : 'rgba(127,127,127,0.18)');
const DOUBLE_TAP_MS = 280;

const NO_SELECT = Platform.OS === 'web' ? ({ userSelect: 'none', WebkitTouchCallout: 'none' } as unknown as ViewStyle) : null;

export function Bubble({
  m,
  author,
  userId,
  fresh,
  onReport,
  onProfile,
  onDelete,
  onRestore,
  onReact,
  onWho,
  onOpen,
}: {
  m: Message;
  author?: Member;
  userId?: number | null;
  fresh: boolean;
  onReport: (target: ModTarget) => void;
  onProfile: (id: number) => void;
  onDelete?: () => Promise<unknown>;
  onRestore?: () => Promise<unknown>;
  onReact?: (emoji: string) => boolean;
  onWho: (emoji: string) => void;
  onOpen: (m: Message) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  const { byId } = useMembers();
  const mine = m.member_id === me?.id;
  const mentionStyle = (mine: boolean, color: string): TextStyle =>
    mine
      ? { fontFamily: t.font.bold, color: '#121216', backgroundColor: 'rgba(0,0,0,0.12)', borderRadius: 5, paddingHorizontal: 3 }
      : { fontFamily: t.font.bold, color, backgroundColor: soft(color), borderRadius: 5, paddingHorizontal: 3 };
  const target = (): ModTarget =>
    mine
      ? { kind: 'message', id: m.id, own: true, onDelete, onRestore }
      : {
          kind: 'message',
          id: m.id,
          person:
            author && userId != null
              ? { user_id: userId, name: author.name, color: author.color, photo: author.photo_url, avatar: author.avatar }
              : undefined,
          onDelete,
          onRestore,
        };
  const bubble = useRef<ComponentRef<typeof View>>(null);
  const lastTap = useRef(0);
  const mentionTap = useRef(0);
  const [burst, setBurst] = useState(0);
  const media = mediaOf(m);
  const opening = useRef<ReturnType<typeof setTimeout>>(undefined);
  useUnmountEffect(() => clearTimeout(opening.current));
  const heart = () => {
    if (onReact?.(HEART)) setBurst((b) => b + 1);
  };
  const onPress =
    onReact || media
      ? () => {
          const now = Date.now();
          if (now - mentionTap.current < 300) return;
          if (onReact && now - lastTap.current < DOUBLE_TAP_MS) {
            lastTap.current = 0;
            clearTimeout(opening.current);
            heart();
            return;
          }
          lastTap.current = now;
          if (!media) return;
          if (onReact) opening.current = setTimeout(() => onOpen(m), DOUBLE_TAP_MS);
          else onOpen(m);
        }
      : undefined;
  const menu = () => {
    lastTap.current = 0;
    impactHaptic();
    const at = target();
    if (!bubble.current) return onReport(at);
    bubble.current.measureInWindow((_x, y, _w, height) =>
      onReport(Number.isFinite(y) && height > 0 ? { ...at, anchor: { y, height, side: mine ? 'right' : 'left' } } : at),
    );
  };
  const onLongPress = onReact || onRestore ? menu : mine ? onDelete && (() => onReport(target())) : () => onReport(target());
  const hint = onReact
    ? tr(mine ? (onDelete ? 'chat.reactions.ownHint' : 'chat.reactions.ownOnlyHint') : 'chat.reactions.hint')
    : mine
      ? onDelete
        ? tr('chat.longPressOwnHint')
        : undefined
      : tr('chat.longPressHint');
  const reactions = m.reactions?.filter((r) => r.count > 0) ?? [];
  const ink = mine ? '#121216' : t.txt;
  const rich = (text: string) => (
    <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 20, color: ink, paddingHorizontal: media ? 10 : 0 }}>
      {segments(
        text,
        (m.mentions ?? []).map((id) => byId.get(id)).filter((x): x is Member => !!x),
        !!m.everyone,
      ).map(({ member, ...p }, i) =>
        member ? (
          <Text
            key={i}
            accessibilityRole="link"
            accessibilityHint={tr('chat.seeProfile')}
            onPress={() => {
              mentionTap.current = Date.now();
              onProfile(member.id);
            }}
            onLongPress={onLongPress}
            style={mentionStyle(mine, member.color)}>
            {p.text}
          </Text>
        ) : p.everyone ? (
          <Text key={i} style={mine ? mentionStyle(true, '') : { ...mentionStyle(false, t.accInk), backgroundColor: t.accSoft }}>
            {p.text}
          </Text>
        ) : (
          p.text
        ),
      )}
    </Text>
  );
  const mediaLabel = media ? tr(m.kind === 'gif' ? 'chat.media.gifBy' : 'chat.media.photoBy', { name: (mine ? me?.name : author?.name) ?? '?' }) : '';
  return (
    <Animated.View key={m.id} entering={fresh ? motion.rise : undefined} style={{ gap: 4 }}>
      <View style={{ flexDirection: mine ? 'row-reverse' : 'row', gap: 8, alignItems: 'flex-end', opacity: m.hidden ? 0.45 : 1 }}>
        {!mine && <Avatar member={author} size={28} />}
        <Pressable
          ref={bubble}
          onPress={onPress}
          onLongPress={onLongPress}
          delayLongPress={400}
          accessibilityLabel={media ? (m.caption ? `${mediaLabel}, ${m.caption}` : mediaLabel) : undefined}
          accessibilityHint={media ? [tr('chat.media.open'), hint].filter(Boolean).join('. ') : hint}
          accessibilityActions={onReact ? [{ name: 'heart', label: tr('chat.reactions.react', { emoji: HEART }) }] : undefined}
          onAccessibilityAction={onReact ? (e) => e.nativeEvent.actionName === 'heart' && heart() : undefined}
          style={[
            NO_SELECT,
            {
              maxWidth: '78%',
              paddingHorizontal: media ? 4 : 14,
              paddingTop: media ? 4 : 9,
              paddingBottom: media ? (m.caption ? 7 : 4) : 9,
              borderRadius: 18,
              borderBottomLeftRadius: mine ? 18 : 6,
              borderBottomRightRadius: mine ? 6 : 18,
              backgroundColor: mine ? (me?.color ?? t.card2) : t.card,
              gap: 2,
            },
          ]}>
          {!mine && (
            <Text
              numberOfLines={1}
              style={{
                fontFamily: t.font.bold,
                fontSize: 12,
                color: author?.color ?? t.mut,
                paddingHorizontal: media ? 10 : 0,
                paddingTop: media ? 3 : 0,
                paddingBottom: media ? 2 : 0,
              }}>
              {author?.name ?? '?'}
            </Text>
          )}
          {media ? (
            <>
              <MessageMediaFrame m={m} media={media} label={mediaLabel} />
              {!!m.caption && rich(m.caption)}
            </>
          ) : (
            rich(m.text)
          )}
          {(!media || !!m.caption) && (
            <Text
              style={{
                fontFamily: t.font.regular,
                fontSize: 10,
                color: mine ? '#121216' : t.mut,
                opacity: 0.7,
                alignSelf: 'flex-end',
                paddingHorizontal: media ? 10 : 0,
              }}>
              {dateFormat({ hour: '2-digit', minute: '2-digit' }).format(new Date(m.created_at))}
            </Text>
          )}
          {!!onReact && <HeartBurst trigger={burst} />}
        </Pressable>
      </View>
      {reactions.length > 0 && <ReactionChips reactions={reactions} mine={mine} onToggle={onReact && ((e) => void onReact(e))} onWho={onWho} />}
      {!!m.hidden && (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 5, alignSelf: mine ? 'flex-end' : 'flex-start', marginLeft: mine ? 0 : 36 }}>
          <EyeOff size={13} color={t.mut} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>{tr('chat.hidden')}</Text>
        </View>
      )}
    </Animated.View>
  );
}
