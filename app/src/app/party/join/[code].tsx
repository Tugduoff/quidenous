import { useMutation, useQuery } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { ArrowRight } from 'lucide-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import { GuestForm, GUEST_COLORS } from '@/components/party/guest-form';
import { JoinInvite } from '@/components/party/join-invite';
import { OtherCode } from '@/components/party/other-code';
import { Avatar } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Screen } from '@/components/ui/screen';
import { ErrorText, Logo, TextLink } from '@/components/ui/text';
import { ApiError } from '@/lib/api/client';
import { partyApi, partyError, partyKeys } from '@/lib/api/party';
import { setPendingCode } from '@/lib/auth-flow';
import { useRedirect } from '@/lib/hooks/use-redirect';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

export default function JoinParty() {
  const t = useTheme();
  const tr = useT();
  const { code: raw } = useLocalSearchParams<{ code: string }>();
  const code = String(raw ?? '')
    .trim()
    .toUpperCase();
  const { token, room, me, joinParty } = useSession();
  const info = useQuery({ queryKey: partyKeys.screen(code), queryFn: () => partyApi.screen(code), retry: false, enabled: !!code });
  const [name, setName] = useState('');
  const [color, setColor] = useState(() => GUEST_COLORS[Math.floor(Math.random() * GUEST_COLORS.length)]);
  const [error, setError] = useState<string | null>(null);
  const [target, setTarget] = useState<number | null>(null);
  const [terms, setTerms] = useState(false);

  useRedirect(target != null && token && room === target ? `/party/${target}` : null);

  const join = useMutation({
    meta: { quiet: true },
    mutationFn: () => partyApi.join(token ? { code } : { code, name: name.trim(), color }),
    onMutate: () => setError(null),
    onSuccess: async (r) => {
      await joinParty(r.token ?? null, r.party_id);
      setTarget(r.party_id);
    },
    onError: (e) => setError(partyError(e)),
  });

  const p = info.data;
  const status = info.error instanceof ApiError ? info.error.code : null;
  const over = p?.phase === 'over' || status === 'party_over';
  const unknown = status === 'not_found';
  const closed = over || unknown;
  const ready = !closed && (!!token || (!!name.trim() && terms)) && !join.isPending;
  const go = () => ready && join.mutate();

  return (
    <Screen
      tabs={false}
      footer={
        closed ? (
          <Button onPress={() => router.replace('/')}>{tr('party.join.open')}</Button>
        ) : (
          <>
            {!!token && me && (
              <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 8 }}>
                <Avatar member={me} size={22} />
                <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
                  {tr('party.join.as', { name: me.name })}
                </Text>
              </View>
            )}
            <ErrorText>{error}</ErrorText>
            <Button disabled={!token && (!name.trim() || !terms)} loading={join.isPending || target != null} onPress={go}>
              <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{tr('party.join.go')}</Text>
              <ArrowRight size={20} color={t.accTxt} />
            </Button>
            {!token && (
              <TextLink
                onPress={() => {
                  setPendingCode(code);
                  router.replace('/join');
                }}>
                {tr('party.join.account')}
              </TextLink>
            )}
          </>
        )
      }>
      <View style={{ marginTop: 24 }}>
        <Logo size={28} />
      </View>

      <JoinInvite code={code} loading={info.isLoading} closed={closed} over={over} party={p} />

      {closed && <OtherCode />}

      {!closed && !token && <GuestForm name={name} onName={setName} color={color} onColor={setColor} terms={terms} onTerms={setTerms} onSubmit={go} />}
    </Screen>
  );
}
