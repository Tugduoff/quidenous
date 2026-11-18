import { router } from 'expo-router';
import { Settings } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { AddFriends, InviteCard } from '@/components/rooms/invite';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Display } from '@/components/ui/text';
import { BackButton } from '@/components/ui/back-button';
import { useGroup } from '@/lib/api/group';
import { useRoomMuted } from '@/lib/hooks/use-room-muted';
import { RoomMembers } from '@/components/rooms/room-members';
import { LeaveRoomButton } from '@/components/rooms/leave-room-button';
import { MuteButton } from '@/components/rooms/mute-button';

export default function GroupScreen() {
  const t = useTheme();
  const tr = useT();
  const groupQuery = useGroup();
  const { data: group } = groupQuery;
  const muted = useRoomMuted(group?.id);
  return (
    <Screen tabs={false}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <BackButton />
        <View style={{ flexDirection: 'row', gap: 8 }}>
          {group && muted !== undefined && <MuteButton id={group.id} muted={muted} />}
          {group?.my_role === 'owner' && (
            <PressableScale
              accessibilityRole="button"
              accessibilityLabel={tr('room.settings.title')}
              onPress={() => router.push('/room-settings')}
              hitSlop={6}
              style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
              <Settings size={20} color={t.txt} />
            </PressableScale>
          )}
        </View>
      </View>
      <View style={{ gap: 4 }}>
        <Display size={30}>{group?.name ?? tr('room.group.fallbackName')}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
          {group ? tr('common.members', { count: group.members.length }) + (muted ? ` · ${tr('room.group.muted')}` : '') : ''}
        </Text>
      </View>
      <QueryState query={groupQuery} />
      <InviteCard rotate={false} />
      <RoomMembers />
      <AddFriends />
      <LeaveRoomButton />
    </Screen>
  );
}
