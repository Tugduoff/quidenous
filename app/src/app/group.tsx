import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { ChevronLeft, UserX } from 'lucide-react-native';
import { Alert, Platform, Pressable, Text, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, Card, Display } from '@/components/ui';
import { api } from '@/lib/api';
import { keys, useGroup } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/** Ask before voting someone out (a vote can be taken back). */
function confirm(message: string): Promise<boolean> {
  if (Platform.OS === 'web') return Promise.resolve(typeof window === 'undefined' || window.confirm(message));
  return new Promise((resolve) =>
    Alert.alert('Exclure du groupe ?', message, [
      { text: 'Annuler', style: 'cancel', onPress: () => resolve(false) },
      { text: 'Voter', style: 'destructive', onPress: () => resolve(true) },
    ]),
  );
}

/** The other members, each with a vote to remove them. A majority of the others removes someone. */
function Members() {
  const t = useTheme();
  const qc = useQueryClient();
  const { me } = useSession();
  const { data: group } = useGroup();
  const kick = useMutation({
    mutationFn: ({ id, undo }: { id: number; undo: boolean }) => api(`/members/${id}/kick`, { method: undo ? 'DELETE' : 'PUT' }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.group }),
  });
  if (!group) return null;
  const others = group.members.filter((m) => m.id !== me?.id);
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>
        {`Exclure quelqu'un demande ${group.kick_needed} vote${group.kick_needed > 1 ? 's' : ''}`}
      </Text>
      {others.map((m) => {
        const state = group.kicks.find((k) => k.target_id === m.id);
        const mine = !!state?.mine;
        const onPress = async () => {
          if (!mine && !(await confirm(`Voter pour exclure ${m.name} ? Avec ${group.kick_needed} votes, ${m.name} sort du groupe.`)))
            return;
          kick.mutate({ id: m.id, undo: mine });
        };
        return (
          <Card key={m.id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
            <Avatar member={m} size={36} />
            <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{m.name}</Text>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={mine ? `Retirer mon vote contre ${m.name}` : `Voter pour exclure ${m.name}`}
              disabled={kick.isPending}
              onPress={onPress}
              hitSlop={6}
              style={{
                flexDirection: 'row',
                alignItems: 'center',
                gap: 6,
                height: 34,
                paddingHorizontal: 12,
                borderRadius: t.radius.pill,
                backgroundColor: mine ? '#FF4FA322' : t.card2,
              }}>
              <UserX size={15} color={mine ? t.tag : t.mut} />
              <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: mine ? t.tag : t.mut }}>
                {state ? `${state.count}/${group.kick_needed}` : 'Exclure'}
              </Text>
            </Pressable>
          </Card>
        );
      })}
    </View>
  );
}

/** The group's own page: its name and its members (my profile stays about me). */
export default function GroupScreen() {
  const t = useTheme();
  const { me } = useSession();
  const { data: group } = useGroup();
  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <View style={{ gap: 4 }}>
        <Display size={30}>{group?.name ?? 'Groupe'}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
          {group ? `${group.members.length} membre${group.members.length > 1 ? 's' : ''}` : ''}
        </Text>
      </View>
      {me && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
          <Avatar member={me} size={36} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{me.name}</Text>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>toi</Text>
        </Card>
      )}
      <Members />
    </Screen>
  );
}
