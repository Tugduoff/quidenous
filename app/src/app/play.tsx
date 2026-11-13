import { useMutation, useQuery } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { ChevronLeft, ChevronRight, Crown, LogIn, Swords } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { RoomForms } from '@/components/room-forms';
import { Screen } from '@/components/screen';
import { Button, Card, Display, Logo, Tag } from '@/components/ui';
import { api, type RoomSummary } from '@/lib/api';
import { pendingPlayToken, setPendingPlay } from '@/lib/auth';
import { communityError, playInto, usePlay } from '@/lib/community';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { keys } from '@/lib/queries';
import { useSwitchRoom } from '@/lib/rooms';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/**
 * `/play?share={token}`: a question shared from another room. Add it to one of my rooms (it comes next there)
 * or create a room with it. Signed out: the question, and sign in first (the token waits, see lib/auth).
 */
export default function PlayScreen() {
  const t = useTheme();
  const tr = useT();
  const { share } = useLocalSearchParams<{ share?: string }>();
  // A deep link carries it as a param; the web read it at load (the router drops the query).
  const [token] = useState(() => {
    const value = share || pendingPlayToken();
    if (value) setPendingPlay(value);
    return value;
  });
  const { token: session, roomless, enterRoom } = useSession();
  const signedIn = !!session;
  const info = usePlay(token);
  // Signed out or roomless there is no list to ask for.
  const { data: rooms } = useQuery({ queryKey: keys.rooms, queryFn: () => api<RoomSummary[]>('/rooms'), enabled: signedIn && !roomless });
  const switchRoom = useSwitchRoom();
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const leave = () => {
    setPendingPlay(null);
    if (router.canGoBack()) router.back();
    else router.replace(!signedIn ? '/join' : roomless ? '/setup' : '/');
  };
  const enter = async (id: number) => {
    setPendingPlay(null);
    if (roomless) {
      await enterRoom(id);
      router.replace('/');
    } else await switchRoom(id);
  };
  // An existing room: the question goes in first, then the app moves there.
  const play = useMutation({
    mutationFn: (id: number) => playInto(token!, id),
    onMutate: () => setError(null),
    onSuccess: (_, id) => enter(id),
    onError: (e) => setError(communityError(e)),
  });
  // A new room: it's mine, so the question always goes in; even if it didn't, the room is made, go there.
  const created = async (id: number) => {
    await playInto(token!, id).catch(() => {});
    await enter(id);
  };

  const back = (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={tr('common.back')}
      onPress={leave}
      style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
      <ChevronLeft size={22} color={t.txt} />
    </Pressable>
  );

  if (!token || info.isError)
    return (
      <Screen tabs={false}>
        {back}
        <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.mut }}>{tr('community.play.gone')}</Text>
      </Screen>
    );
  if (!info.data)
    return (
      <Screen tabs={false}>
        {back}
        <ActivityIndicator color={t.mut} style={{ marginTop: 40 }} />
      </Screen>
    );
  const q = info.data;

  return (
    <Screen
      tabs={false}
      footer={
        !signedIn && (
          <Button onPress={() => router.push('/join')}>
            <LogIn size={20} color={t.accTxt} />
            <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{tr('community.play.signIn')}</Text>
          </Button>
        )
      }>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        {back}
        <Logo />
      </View>
      {!creating && (
        <Animated.View entering={motion.question} style={{ gap: 12 }}>
          <Tag label={tr(q.kind === 'rank' ? 'vote.kind.rank' : 'vote.kind.pick')} />
          <Display size={30}>{q.question}</Display>
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
            {q.closed && <Swords size={14} color={t.mut} />}
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
              {q.closed ? tr('community.play.challenge') : tr('community.play.from', { room: q.room })}
            </Text>
          </View>
        </Animated.View>
      )}

      {signedIn && !creating && !!rooms?.length && (
        <View style={{ gap: 10, marginTop: 8 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{tr('community.play.pick')}</Text>
          {rooms.map((r) => {
            const busy = play.isPending && play.variables === r.id;
            return (
              <Pressable key={r.id} accessibilityRole="button" disabled={play.isPending} onPress={() => play.mutate(r.id)}>
                <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
                  <View style={{ flex: 1, gap: 2 }}>
                    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
                        {r.name}
                      </Text>
                      {r.role === 'owner' && <Crown size={14} color={t.accInk} />}
                    </View>
                    <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
                      {tr('common.members', { count: r.members })}
                    </Text>
                  </View>
                  {busy ? <ActivityIndicator color={t.mut} /> : <ChevronRight size={20} color={t.mut} />}
                </Card>
              </Pressable>
            );
          })}
          {error && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag, marginLeft: 4 }}>{error}</Text>}
        </View>
      )}

      {signedIn && <RoomForms createOnly={tr('community.play.newRoom')} onEntered={created} onVibeStep={setCreating} />}
    </Screen>
  );
}
