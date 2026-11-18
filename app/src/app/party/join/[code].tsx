import { useMutation, useQuery } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { ArrowRight, Check, PartyPopper } from 'lucide-react-native';
import { useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Screen } from '@/components/ui/screen';
import { TermsCheck } from '@/components/auth/terms';
import { ApiError } from '@/lib/api/client';
import { PARTS } from '@/lib/utils/avatar';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { partyApi, partyError, partyKeys } from '@/lib/api/party';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar, AvatarStack } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Display, Logo, ErrorText, TextLink } from '@/components/ui/text';
import { setPendingCode } from '@/lib/auth-flow';

/** The avatar backgrounds people already pick from, minus the dark one (the initial drawn on it would not show). */
const COLORS = PARTS.bg.map(([c]) => c).filter((c) => c.toUpperCase() !== '#2E2E38');

/**
 * `/party/join/[code]`: the link the party's QR code holds. Signed in: one tap. Without an account: a first name
 * and a colour make a guest account (its token is kept like any session), then straight into the party.
 */
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
  const [color, setColor] = useState(() => COLORS[Math.floor(Math.random() * COLORS.length)]);
  const [error, setError] = useState<string | null>(null);
  const [target, setTarget] = useState<number | null>(null);
  const [terms, setTerms] = useState(false);
  // A bad or ended link: another code to try.
  const [other, setOther] = useState('');

  // Once the session is in that party (a guest's goes from signed out to in), on to its screen.
  useEffect(() => {
    if (target != null && token && room === target) router.replace(`/party/${target}`);
  }, [target, token, room]);

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
  // A guest account is made here: the terms first.
  const ready = !closed && (!!token || (!!name.trim() && terms)) && !join.isPending;
  const go = () => ready && join.mutate();
  const otherCode = other.trim().toUpperCase();
  const tryOther = () => {
    if (otherCode.length < 4) return;
    setOther('');
    router.replace(`/party/join/${encodeURIComponent(otherCode)}`);
  };

  return (
    <Screen
      tabs={false}
      footer={
        closed ? (
          // Nothing to join here: on to the app (home, or the welcome screen without an account).
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

      <Animated.View entering={motion.question} style={{ gap: 10 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <PartyPopper size={18} color={t.accInk} />
          <Text style={{ fontFamily: t.font.bold, fontSize: 15, letterSpacing: 2, color: t.accInk }}>{code}</Text>
        </View>
        {info.isLoading ? (
          <ActivityIndicator color={t.mut} style={{ alignSelf: 'flex-start' }} />
        ) : (
          <Display size={32}>{closed ? tr(over ? 'party.errors.over' : 'party.errors.notFound') : p?.name || tr('party.join.title')}</Display>
        )}
        {!closed && !!p?.players.length && (
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
            <AvatarStack members={p.players} size={28} max={6} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('party.lobby.players', { count: p.players.length })}</Text>
          </View>
        )}
      </Animated.View>

      {closed && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16, marginTop: 8 }}>
          <TextInput
            accessibilityLabel={tr('party.join.other')}
            placeholder={tr('party.join.other')}
            placeholderTextColor={t.mut}
            value={other}
            onChangeText={(v) => setOther(v.replace(/[^a-z0-9]/gi, '').toUpperCase())}
            maxLength={12}
            autoCapitalize="characters"
            autoCorrect={false}
            returnKeyType="go"
            onSubmitEditing={tryOther}
            style={{ flex: 1, minWidth: 0, height: 48, fontFamily: t.font.bold, fontSize: 17, letterSpacing: other ? 2 : 0, color: t.txt }}
          />
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('party.join.go')}
            disabled={otherCode.length < 4}
            onPress={tryOther}
            style={{
              width: 44,
              height: 44,
              borderRadius: 22,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: otherCode.length >= 4 ? t.acc : t.card2,
            }}>
            <ArrowRight size={20} color={otherCode.length >= 4 ? t.accTxt : t.mut} />
          </PressableScale>
        </Card>
      )}

      {!closed && !token && (
        <View style={{ gap: 16, marginTop: 8 }}>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 8, paddingLeft: 10 }}>
            <Avatar member={{ id: 0, name: name.trim() || '?', color, photo_url: null }} size={44} />
            <TextInput
              accessibilityLabel={tr('party.join.name')}
              placeholder={tr('party.join.name')}
              placeholderTextColor={t.mut}
              value={name}
              onChangeText={setName}
              maxLength={24}
              autoComplete="given-name"
              autoCapitalize="words"
              returnKeyType="go"
              onSubmitEditing={go}
              style={{ flex: 1, minWidth: 0, height: 48, fontFamily: t.font.semibold, fontSize: 17, color: t.txt }}
            />
          </Card>
          <View style={{ gap: 10 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{tr('party.join.color')}</Text>
            <View accessibilityRole="radiogroup" style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8 }}>
              {COLORS.map((c, i) => {
                const on = c === color;
                return (
                  <Pressable
                    key={c}
                    accessibilityRole="radio"
                    accessibilityLabel={tr('party.join.colorA11y', { n: i + 1 })}
                    accessibilityState={{ selected: on }}
                    onPress={() => setColor(c)}
                    style={{
                      width: 48,
                      height: 48,
                      borderRadius: 24,
                      alignItems: 'center',
                      justifyContent: 'center',
                      borderWidth: 2,
                      borderColor: on ? t.txt : 'transparent',
                    }}>
                    <View style={{ width: 36, height: 36, borderRadius: 18, backgroundColor: c, alignItems: 'center', justifyContent: 'center' }}>
                      {on && <Check size={18} color="#121216" />}
                    </View>
                  </Pressable>
                );
              })}
            </View>
          </View>
          <TermsCheck value={terms} onChange={setTerms} />
        </View>
      )}
    </Screen>
  );
}
