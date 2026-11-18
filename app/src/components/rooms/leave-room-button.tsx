import { LogOut } from 'lucide-react-native';
import { Pressable, Text } from 'react-native';
import { showToast } from '@/lib/toast';
import { useT } from '@/lib/i18n';
import { useRoomGone } from '@/lib/hooks/use-switch-room';
import { useTheme } from '@/lib/theme';
import { askConfirm } from '@/lib/confirm';
import { useGroup, leaveRoom } from '@/lib/api/group';
import { authError } from '@/lib/api/errors';

const confirm = (message: string, title: string, action: string) => askConfirm(title, message, action);

export function LeaveRoomButton() {
  const t = useTheme();
  const tr = useT();
  const roomGone = useRoomGone();
  const { data: group } = useGroup();
  if (!group) return null;
  const owner = group.my_role === 'owner';
  const leave = async () => {
    const msg = owner && group.members.length <= 1 ? tr('room.group.leaveAlone') : owner ? tr('room.group.leaveOwner') : tr('room.group.leaveMember');
    if (!(await confirm(msg, tr('room.group.leaveAsk'), tr('common.leave')))) return;
    try {
      await leaveRoom(group.id);
    } catch (e) {
      return showToast(authError(e));
    }
    await roomGone();
  };
  return (
    <Pressable
      accessibilityRole="button"
      onPress={leave}
      style={{
        height: 48,
        marginTop: 8,
        borderRadius: t.radius.pill,
        flexDirection: 'row',
        gap: 8,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: '#FF4FA318',
      }}>
      <LogOut size={18} color={t.tagInk} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.tagInk }}>{tr('room.group.leave')}</Text>
    </Pressable>
  );
}
