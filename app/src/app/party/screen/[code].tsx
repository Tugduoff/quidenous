import { useQuery } from '@tanstack/react-query';
import { useLocalSearchParams } from 'expo-router';
import { ActivityIndicator, ScrollView, Text, useWindowDimensions, View } from 'react-native';
import { JoinQr } from '@/components/party/party';
import { ScreenStage } from '@/components/party/screen-stage';
import { ScreenBursts } from '@/components/result/live-reactions';
import { CodeText, Logo } from '@/components/ui/text';
import { useFeatures } from '@/lib/api/features';
import { partyApi, partyError, partyKeys } from '@/lib/api/party';
import { useScreenLive } from '@/lib/hooks/use-live';
import { useGuestScreenLanguage } from '@/lib/hooks/use-party';
import { useT } from '@/lib/i18n';
import { partyJoinLink } from '@/lib/links';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

export default function PartyBigScreen() {
  const t = useTheme();
  const tr = useT();
  const { code: raw } = useLocalSearchParams<{ code: string }>();
  const code = String(raw ?? '')
    .trim()
    .toUpperCase();
  const { width, height } = useWindowDimensions();
  const { resultReactions } = useFeatures();
  const q = useQuery({ queryKey: partyKeys.screen(code), queryFn: () => partyApi.screen(code), refetchInterval: 15_000, enabled: !!code });
  useScreenLive(code, () => q.refetch());
  const p = q.data;
  const { token } = useSession();
  useGuestScreenLanguage(p?.language, !!token);
  const wide = width >= 760 && width > height * 1.1;
  const unit = Math.min(width / (wide ? 1600 : 800), height / 900);

  if (!p)
    return (
      <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center', gap: 20, backgroundColor: t.bg }}>
        <Logo size={40} />
        {q.error ? (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 22, color: t.mut }}>{partyError(q.error)}</Text>
        ) : (
          <ActivityIndicator color={t.mut} />
        )}
      </View>
    );

  const said = p.round?.kind === 'said' ? p.round.said : null;
  const full = !!said && said.phase !== 'writing' && (p.phase === 'voting' || p.phase === 'reveal');
  const qr = Math.round(Math.max(160, Math.min(wide ? height * 0.42 : width * 0.5, 420)));
  const side = (
    <View style={{ alignItems: 'center', gap: 18, width: wide ? qr + 80 : '100%' }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 22, color: t.mut }}>{tr('party.screen.join')}</Text>
      <JoinQr code={p.code} size={qr} />
      <View style={{ width: qr + 40 }}>
        <CodeText code={p.code} size={72} center />
      </View>
      <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 16, color: t.mut }}>
        {partyJoinLink(p.code).replace(/^https?:\/\//, '')}
      </Text>
    </View>
  );

  return (
    <View style={{ flex: 1, backgroundColor: t.bg }}>
      <ScrollView style={{ flex: 1 }} contentContainerStyle={{ flexGrow: 1, padding: wide ? 48 : 24, gap: 32 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: 20 }}>
          <Logo size={28} />
          <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 22, color: t.mut }}>
            {p.name || tr('party.name')}
            {full && (
              <Text style={{ fontFamily: t.font.semibold }}>
                {'  ·  '}
                {tr('said.party.code')} <Text style={{ fontFamily: t.font.display, color: t.txt, letterSpacing: 2 }}>{p.code}</Text>
              </Text>
            )}
          </Text>
        </View>
        <View style={{ flex: 1, flexDirection: wide ? 'row' : 'column-reverse', alignItems: wide ? 'center' : 'stretch', gap: 48 }}>
          {!full && side}
          <View style={{ flex: wide ? 1 : undefined, minWidth: 0, justifyContent: 'center', gap: 32 }}>
            <ScreenStage party={p} unit={Math.max(0.6, unit)} wide={wide} reactions={resultReactions} />
          </View>
        </View>
      </ScrollView>
      {resultReactions && p.phase === 'reveal' && p.round && <ScreenBursts roundId={p.round.id} wide={wide} />}
    </View>
  );
}
