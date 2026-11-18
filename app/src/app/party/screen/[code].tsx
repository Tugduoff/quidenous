import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useLocalSearchParams } from 'expo-router';
import { useEffect } from 'react';
import { ActivityIndicator, ScrollView, Text, useWindowDimensions, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { ScreenBursts, ScreenCounts } from '@/components/result/live-reactions';
import { Mascot } from '@/components/mascot/mascot';
import { MascotBand } from '@/components/mascot/band';
import { Countdown, JoinQr, PlayerChips, Reactions, VotedLine, WinnerReveal } from '@/components/party/party';
import { SaidScreenQuestion, SaidScreenResult } from '@/components/party/said';
import { ResultBars } from '@/components/result/result-bars';
import type { Member } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { showLang, useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { motion } from '@/lib/motion';
import { type Party, partyApi, partyError, partyJoinLink, partyKeys, type PartyRound, tallies, useCountdown, useRefetchAtZero, useScreenLive } from '@/lib/party';
import { tidy } from '@/lib/text';
import { useTheme } from '@/lib/theme';
import { Tag } from '@/components/ui/tag';
import { CodeText, Display, Logo } from '@/components/ui/text';

/**
 * `/party/screen/[code]`: the party on a TV or a laptop, no account. The QR code and the code to join on one side,
 * the question, countdown, vote count and the big reveal on the other. Live from the party's socket.
 */
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
  // A TV has no account: it speaks the party's language. A signed-in phone keeps its own.
  const { token } = useSession();
  useEffect(() => {
    if (p?.language && !token) showLang(p.language);
  }, [p?.language, token]);
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

  // A said question's answers and reveal take the whole screen (nobody joins mid-question anyway): the code goes on top.
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
            <Stage party={p} unit={Math.max(0.6, unit)} wide={wide} reactions={resultReactions} />
          </View>
        </View>
      </ScrollView>
      {/* Taps from the phones float up from the bottom edge, over everything. */}
      {resultReactions && p.phase === 'reveal' && p.round && <ScreenBursts roundId={p.round.id} wide={wide} />}
    </View>
  );
}

const byIdOf = (p: Party) => new Map<number, Member>(p.players.map((m) => [m.id, m]));

/** What the room is looking at: the players while waiting, the question, the result, the end. */
function Stage({ party, unit, wide, reactions }: { party: Party; unit: number; wide: boolean; reactions: boolean }) {
  const t = useTheme();
  const tr = useT();
  const big = (n: number) => Math.round(n * unit);
  const said = party.round?.kind === 'said' ? party.round.said : null;
  if (party.phase === 'voting' && party.round && said)
    return <SaidScreenQuestion key={`${party.round.id}-${said.phase}`} party={party} round={party.round} said={said} unit={unit} wide={wide} />;
  if (party.phase === 'reveal' && party.round && said)
    return <SaidScreenResult key={party.round.id} party={party} round={party.round} said={said} unit={unit} wide={wide} reactions={reactions} />;
  if (party.phase === 'voting' && party.round) return <Question key={party.round.id} party={party} round={party.round} unit={unit} />;
  if (party.phase === 'reveal' && party.round) return <Result key={party.round.id} party={party} round={party.round} unit={unit} wide={wide} reactions={reactions} />;
  if (party.phase === 'over')
    return (
      <Animated.View entering={motion.enter} style={{ gap: 16 }}>
        <Mascot who="paillette" mood="celebrate" size={big(240)} style={{ marginLeft: -big(24) }} />
        <Display size={big(96)}>{tr('party.over')}</Display>
        <Text style={{ fontFamily: t.font.semibold, fontSize: big(32), color: t.mut }}>{tr('party.memory.played', { count: party.played })}</Text>
      </Animated.View>
    );
  return (
    <View style={{ gap: 28 }}>
      {/* The lobby: the whole band, pointing at whoever is next. */}
      <MascotBand max={big(760)} style={{ alignItems: 'flex-start' }} />
      <Display size={big(80)}>{party.players.length < 2 ? tr('party.screen.waiting') : tr('party.screen.ready')}</Display>
      <PlayerChips players={party.players} big />
    </View>
  );
}

function Question({ party, round, unit }: { party: Party; round: PartyRound; unit: number }) {
  const ms = useCountdown(round.deadline_at, party.clock);
  const qc = useQueryClient();
  useRefetchAtZero(ms, () => qc.invalidateQueries({ queryKey: partyKeys.screen(party.code) }));
  const size = Math.round((round.question.length > 90 ? 64 : 84) * unit);
  return (
    <Animated.View entering={motion.question} style={{ gap: 36 * unit }}>
      <Tag label={`#${round.number}`} />
      <Display size={size}>{tidy(round.question)}</Display>
      <Countdown ms={ms} total={party.timer} big />
      <VotedLine round={round} byId={byIdOf(party)} big />
    </Animated.View>
  );
}

/** The result: on a wide screen the winner and the bars side by side, so it all fits without scrolling. */
function Result({ party, round, unit, wide, reactions }: { party: Party; round: PartyRound; unit: number; wide: boolean; reactions: boolean }) {
  const t = useTheme();
  const byId = byIdOf(party);
  const list = tallies(round).slice(0, wide ? 4 : 5);
  const bars = list.length > 1 && <ResultBars result={list} byId={byId} />;
  return (
    <View style={{ gap: 32 * unit }}>
      <Text numberOfLines={3} style={{ fontFamily: t.font.bold, fontSize: Math.round(34 * unit), lineHeight: Math.round(42 * unit), color: t.txt }}>
        {tidy(round.question)}
      </Text>
      <View style={{ flexDirection: wide && bars ? 'row' : 'column', alignItems: wide ? 'center' : 'stretch', gap: 40 * unit }}>
        <View style={{ flex: wide && bars ? 1 : undefined }}>
          <WinnerReveal round={round} byId={byId} size={Math.round(180 * unit)} />
        </View>
        {bars && <View style={{ flex: wide ? 1.3 : undefined, minWidth: 0 }}>{bars}</View>}
      </View>
      {reactions ? <ScreenCounts reactions={round.reactions} unit={unit} /> : !!round.reactions?.length && <Reactions round={round} disabled />}
    </View>
  );
}
