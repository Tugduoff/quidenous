import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Check, Clock, UserPlus, X } from 'lucide-react-native';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { t as tr, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Display } from '@/components/ui/text';
import { BackButton } from '@/components/ui/back-button';
import { useFriends, friendKeys, keepFriend, removeFriend } from '@/lib/api/friends';
import { FriendsLabel, PersonRow, RoundAction } from '@/components/friends/person-row';
import { FriendCodeCard } from '@/components/friends/friend-code-card';
import { AddFriendCard } from '@/components/friends/add-friend-card';

const roomsText = (n: number) => tr('friends.roomsInCommon', { count: n });

export default function FriendsScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const friendsQuery = useFriends();
  const data = friendsQuery.data;
  const refresh = () => qc.invalidateQueries({ queryKey: friendKeys.all });
  const keep = useMutation({
    mutationFn: (id: number) => keepFriend(id),
    onSettled: refresh,
  });
  const remove = useMutation({
    mutationFn: (id: number) => removeFriend(id),
    onSettled: refresh,
  });
  const busy = keep.isPending || remove.isPending;
  const friends = data?.friends.filter((f) => f.added) ?? [];
  const roomMates = data?.friends.filter((f) => !f.added) ?? [];
  const shown = new Set(data?.friends.map((f) => f.user_id));
  const incoming = data?.incoming ?? [];
  const outgoing = data?.outgoing?.filter((p) => !shown.has(p.user_id)) ?? [];

  return (
    <Screen tabs={false}>
      <BackButton />
      <Display size={30}>{tr('friends.title')}</Display>
      <QueryState query={friendsQuery} />

      {!!incoming.length && <FriendsLabel>{tr('friends.requests')}</FriendsLabel>}
      {incoming.map((p) => (
        <PersonRow key={p.user_id} person={p} sub={tr('friends.wantsToBeFriends')}>
          <RoundAction label={tr('friends.declineA11y', { name: p.name })} disabled={busy} onPress={() => remove.mutate(p.user_id)}>
            <X size={16} color={t.mut} />
          </RoundAction>
          <RoundAction label={tr('friends.acceptA11y', { name: p.name })} disabled={busy} onPress={() => keep.mutate(p.user_id)} tone="acc">
            <Check size={18} color={t.accTxt} />
          </RoundAction>
        </PersonRow>
      ))}

      {data && <FriendCodeCard code={data.code} />}

      <AddFriendCard onAdded={refresh} />

      {!!friends.length && <FriendsLabel>{tr('friends.count', { count: friends.length })}</FriendsLabel>}
      {friends.map((f) => (
        <PersonRow key={f.user_id} person={f} sub={f.rooms_in_common ? roomsText(f.rooms_in_common) : tr('friends.addedAsFriend')}>
          {!f.rooms_in_common && (
            <RoundAction label={tr('friends.removeA11y', { name: f.name })} disabled={busy} onPress={() => remove.mutate(f.user_id)}>
              <X size={16} color={t.mut} />
            </RoundAction>
          )}
        </PersonRow>
      ))}

      {!!roomMates.length && <FriendsLabel>{tr('friends.inRooms')}</FriendsLabel>}
      {roomMates.map((f) =>
        f.requested ? (
          <PersonRow key={f.user_id} person={f} sub={tr('friends.pending')}>
            <RoundAction label={tr('friends.cancelA11y', { name: f.name })} disabled={busy} onPress={() => remove.mutate(f.user_id)}>
              <Clock size={16} color={t.mut} />
            </RoundAction>
          </PersonRow>
        ) : (
          <PersonRow key={f.user_id} person={f} sub={roomsText(f.rooms_in_common)}>
            <RoundAction label={tr('friends.addNameA11y', { name: f.name })} disabled={busy} onPress={() => keep.mutate(f.user_id)}>
              <UserPlus size={16} color={t.txt} />
            </RoundAction>
          </PersonRow>
        ),
      )}

      {!!outgoing.length && <FriendsLabel>{tr('friends.sent')}</FriendsLabel>}
      {outgoing.map((p) => (
        <PersonRow key={p.user_id} person={p} sub={tr('friends.pending')}>
          <RoundAction label={tr('friends.cancelA11y', { name: p.name })} disabled={busy} onPress={() => remove.mutate(p.user_id)}>
            <X size={16} color={t.mut} />
          </RoundAction>
        </PersonRow>
      ))}
    </Screen>
  );
}
