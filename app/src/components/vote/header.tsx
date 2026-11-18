import { useTheme } from '@/lib/theme';
import { useT } from '@/lib/i18n';
import { useState, useEffect } from 'react';
import { type PushState, pushState, refreshPush, enablePush } from '@/lib/device/web-push';
import { Pressable, Text, View } from 'react-native';
import { BellOff, Bell, ChevronDown, Inbox, Users } from 'lucide-react-native';
import { useGroup } from '@/lib/api/group';
import { useSecretPicker } from '@/components/celebration';
import { router } from 'expo-router';
import { useInbox } from '@/lib/api/inbox';
import { PressableScale } from '@/components/ui/pressable-scale';
import { TAG_BG } from '@/components/ui/tag';
import { SecureCard } from '@/components/security/secure-card';
import { GlobalCard } from '@/components/global/global';
import { useSession } from '@/lib/session';
import { Logo } from '@/components/ui/text';
import { RoomStreak } from '@/components/streaks/streaks';
import { Avatar } from '@/components/ui/avatar';

function PushBell() {
  const t = useTheme();
  const tr = useT();
  const [state, setState] = useState<PushState>(pushState);
  useEffect(() => {
    refreshPush().catch(() => {});
  }, []);
  // Only shown while notifications can still be turned on.
  if (state === 'unsupported' || state === 'granted') return null;
  const denied = state === 'denied';
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={tr('vote.header.enablePush')}
      disabled={denied}
      onPress={() =>
        enablePush()
          .then(setState)
          .catch(() => setState(pushState()))
      }
      hitSlop={4}
      style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: denied ? t.card : t.acc }}>
      {denied ? <BellOff size={18} color={t.mut} /> : <Bell size={18} color={t.accTxt} />}
    </Pressable>
  );
}

/** Which room I'm in; tap to switch, create or join one. */
function RoomPill() {
  const t = useTheme();
  const tr = useT();
  const { data: group } = useGroup();
  const secret = useSecretPicker();
  if (!group) return null;
  return (
    <>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('vote.header.room', { name: group.name })}
        onPress={() => router.push('/rooms')}
        onLongPress={secret.onLongPress}
        hitSlop={6}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 4, alignSelf: 'flex-start', maxWidth: '100%', flexShrink: 1 }}>
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
          {group.name}
        </Text>
        <ChevronDown size={14} color={t.mut} />
      </Pressable>
      {secret.picker}
    </>
  );
}

/** The in-app notifications, with a badge for what I have not seen. */
function InboxButton() {
  const t = useTheme();
  const tr = useT();
  const { data } = useInbox();
  const unread = data?.unread ?? 0;
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={unread ? tr('vote.header.inboxUnread', { count: unread }) : tr('vote.header.inbox')}
      onPress={() => router.push('/inbox')}
      hitSlop={4}
      style={[{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
      <Inbox size={18} color={t.txt} />
      {unread > 0 && (
        <View
          style={{
            position: 'absolute',
            top: -3,
            right: -3,
            minWidth: 18,
            height: 18,
            paddingHorizontal: 4,
            borderRadius: 9,
            backgroundColor: TAG_BG,
            alignItems: 'center',
            justifyContent: 'center',
          }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 10, color: '#fff' }}>{unread > 9 ? '9+' : unread}</Text>
        </View>
      )}
    </PressableScale>
  );
}

/** Opens the group page (members, and later invites and settings). */
function GroupButton() {
  const t = useTheme();
  const tr = useT();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={tr('vote.header.group')}
      onPress={() => router.push('/group')}
      hitSlop={4}
      style={[{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
      <Users size={18} color={t.txt} />
    </PressableScale>
  );
}

export function Header() {
  return (
    <>
      <HeaderBar />
      <SecureCard />
      <GlobalCard />
    </>
  );
}

function HeaderBar() {
  const tr = useT();
  const { me } = useSession();
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', gap: 12 }}>
      <View style={{ gap: 4, flexShrink: 1 }}>
        <Logo />
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <RoomPill />
          <RoomStreak />
        </View>
      </View>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <GroupButton />
        <InboxButton />
        <PushBell />
        <Pressable accessibilityRole="button" accessibilityLabel={tr('vote.header.profile')} onPress={() => router.push('/profile')}>
          <Avatar member={me ?? undefined} size={36} />
        </Pressable>
      </View>
    </View>
  );
}
