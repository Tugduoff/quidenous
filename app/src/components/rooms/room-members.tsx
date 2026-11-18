import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Crown, Ellipsis, UserX } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { type ModAction, ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { StreakBadge } from '@/components/streaks/streaks';
import { MemberTitle } from '@/components/badges/title-chip';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { useGroup, groupKeys, removeMember, voteKick } from '@/lib/api/group';

/** The "…" of a member row (36 px): remove, vote to remove, report, block. */
function MoreButton({ label, onPress }: { label: string; onPress: () => void }) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={label}
      onPress={onPress}
      hitSlop={4}
      style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
      <Ellipsis size={16} color={t.mut} />
    </PressableScale>
  );
}

/**
 * The members, each with a "…": the owner removes someone there; with the vote on, the others vote there to remove them
 * (a majority of the others, never the owner, and only from 3 people). A vote in progress shows as a quiet count on the row.
 */
export function RoomMembers() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { me } = useSession();
  const { data: group } = useGroup();
  const [target, setTarget] = useState<ModTarget | null>(null);
  const owner = group?.my_role === 'owner';
  const forceRemove = useMutation({
    mutationFn: (id: number) => (group ? removeMember(group.id, id) : Promise.reject(new Error('no room'))),
    onSettled: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
  });
  const kick = useMutation({
    mutationFn: ({ id, undo }: { id: number; undo: boolean }) => voteKick(id, !undo),
    onSettled: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
  });
  if (!group) return null;
  // With two people a majority is one person deciding alone: no vote then.
  const voteOn = group.settings.kick_vote && group.members.length >= 3;
  const others = group.members.filter((m) => m.id !== me?.id);
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{tr('room.group.members')}</Text>
      {me && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
          <Avatar member={me} size={36} />
          <View style={{ flex: 1, gap: 3 }}>
            <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
              {me.name}
            </Text>
            <MemberTitle id={me.id} />
          </View>
          <StreakBadge id={me.id} />
          {owner && <Crown size={16} color={t.accInk} />}
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('room.group.you')}</Text>
        </Card>
      )}
      {others.map((m) => {
        const state = voteOn ? group.kicks.find((k) => k.target_id === m.id) : undefined;
        const mine = !!state?.mine;
        const votes = state ? `${state.count}/${group.kick_needed}` : undefined;
        const actions: ModAction[] = owner
          ? [
              {
                icon: UserX,
                label: tr('room.group.removeA11y', { name: m.name }),
                danger: true,
                confirm: { title: tr('room.group.removeAsk'), message: tr('room.group.removeMessage', { name: m.name }), label: tr('room.group.remove') },
                run: () => forceRemove.mutateAsync(m.id),
              },
            ]
          : voteOn && m.role !== 'owner'
            ? [
                {
                  icon: UserX,
                  label: tr(mine ? 'room.group.unvoteA11y' : 'room.group.voteA11y', { name: m.name }),
                  hint: votes,
                  danger: !mine,
                  confirm: mine
                    ? undefined
                    : {
                        title: tr('room.group.kickAsk', { name: m.name }),
                        message: tr('room.group.kickMessage', { name: m.name, count: group.kick_needed }),
                        label: tr('room.group.vote'),
                      },
                  run: () => kick.mutateAsync({ id: m.id, undo: mine }),
                },
              ]
            : [];
        return (
          <Card key={m.id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
            <Avatar member={m} size={36} />
            <View style={{ flex: 1, gap: 3 }}>
              <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                {m.name}
              </Text>
              <MemberTitle id={m.id} />
            </View>
            <StreakBadge id={m.id} />
            {m.role === 'owner' && <Crown size={16} color={t.accInk} />}
            {state && (
              <View
                accessible
                accessibilityLabel={tr('room.group.kickVotesA11y', { count: state.count, total: group.kick_needed })}
                style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
                <UserX size={13} color={mine ? t.tagInk : t.mut} />
                <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: mine ? t.tagInk : t.mut }}>{votes}</Text>
              </View>
            )}
            <MoreButton
              label={tr('room.group.moreA11y', { name: m.name })}
              onPress={() =>
                setTarget({
                  kind: 'member',
                  id: m.id,
                  person: { user_id: m.user_id, name: m.name, color: m.color, photo: m.photo_url, avatar: m.avatar },
                  actions,
                })
              }
            />
          </Card>
        );
      })}
      <ModerationDialog target={target} onClose={() => setTarget(null)} />
    </View>
  );
}
