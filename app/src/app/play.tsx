import { useMutation } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { LogIn } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import { PlayRoomList } from '@/components/play/play-room-list';
import { SharedQuestion } from '@/components/play/shared-question';
import { RoomForms } from '@/components/rooms/room-forms';
import { BackButton } from '@/components/ui/back-button';
import { Screen } from '@/components/ui/screen';
import { communityError, playInto, usePlay } from '@/lib/api/community';
import { useT } from '@/lib/i18n';
import { useSwitchRoom } from '@/lib/hooks/use-switch-room';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Logo } from '@/components/ui/text';
import { useRooms } from '@/lib/api/rooms';
import { pendingPlayToken, setPendingPlay } from '@/lib/auth-flow';

export default function PlayScreen() {
  const t = useTheme();
  const tr = useT();
  const { share } = useLocalSearchParams<{ share?: string }>();
  const [token] = useState(() => {
    const value = share || pendingPlayToken();
    if (value) setPendingPlay(value);
    return value;
  });
  const { token: session, roomless, enterRoom } = useSession();
  const signedIn = !!session;
  const info = usePlay(token);
  const { data: rooms } = useRooms(signedIn && !roomless);
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
  const playHere = (id: number) => (token ? playInto(token, id) : Promise.reject(new Error('no shared question')));
  const play = useMutation({
    meta: { quiet: true },
    mutationFn: playHere,
    onMutate: () => setError(null),
    onSuccess: (_, id) => enter(id),
    onError: (e) => setError(communityError(e)),
  });
  const created = async (id: number) => {
    await playHere(id).catch(() => {});
    await enter(id);
  };

  const back = <BackButton onPress={leave} />;

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
      {!creating && <SharedQuestion q={q} />}

      {signedIn && !creating && !!rooms?.length && (
        <PlayRoomList rooms={rooms} pending={play.isPending} busyId={play.variables} error={error} onPick={play.mutate} />
      )}

      {signedIn && <RoomForms createOnly={tr('community.play.newRoom')} onEntered={created} onVibeStep={setCreating} />}
    </Screen>
  );
}
