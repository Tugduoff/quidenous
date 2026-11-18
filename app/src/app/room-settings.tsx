import { Text, View } from 'react-native';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { useT } from '@/lib/i18n';
import { useGoBackWhen } from '@/lib/hooks/use-redirect';
import { useTheme } from '@/lib/theme';
import { Display } from '@/components/ui/text';
import { BackButton } from '@/components/ui/back-button';
import { RoomSettingsForm } from '@/components/rooms/room-settings-form';
import { useGroup } from '@/lib/api/group';

/** Group > gear (owner only): the room's name, tone, pacing, votes, who adds questions, the invite code, and the danger zone. */
export default function RoomSettings() {
  const t = useTheme();
  const tr = useT();
  const groupQuery = useGroup();
  const group = groupQuery.data;
  const owner = group?.my_role === 'owner';
  // Not (or no longer) the owner: nothing to see here.
  useGoBackWhen(!!group && !owner, '/group');
  return (
    <Screen tabs={false}>
      <BackButton />
      <View style={{ gap: 4 }}>
        <Display size={30}>{tr('room.settings.title')}</Display>
        {owner && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
            {group.name}
          </Text>
        )}
      </View>
      {!group ? <QueryState query={groupQuery} /> : owner ? <RoomSettingsForm group={group} /> : null}
    </Screen>
  );
}
