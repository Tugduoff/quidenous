import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useLocalSearchParams } from 'expo-router';
import { ActivityIndicator, ScrollView, Text, useWindowDimensions, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { Countdown, JoinQr, PlayerChips, Reactions, VotedLine, WinnerReveal } from '@/components/party';
import { ResultBars } from '@/components/result-bars';
import { CodeText, Display, Logo, Tag } from '@/components/ui';
import type { Member } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import {
  type Party,
  partyApi,
  partyError,
  partyJoinLink,
  partyKeys,
  type PartyRound,
  tallies,
  tidy,
  useCountdown,
  useRefetchAtZero,
  useScreenLive,
} from '@/lib/party';
import { useTheme } from '@/lib/theme';

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
  const q = useQuery({ queryKey: partyKeys.screen(code), queryFn: () => partyApi.screen(code), refetchInterval: 15_000, enabled: !!code });
  useScreenLive(code, () => q.refetch());
  const p = q.data;
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
    <ScrollView style={{ flex: 1, backgroundColor: t.bg }} contentContainerStyle={{ flexGrow: 1, padding: wide ? 48 : 24, gap: 32 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: 20 }}>
        <Logo size={28} />
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 22, color: t.mut }}>
          {p.name || tr('party.name')}
        </Text>
      </View>
      <View style={{ flex: 1, flexDirection: wide ? 'row' : 'column-reverse', alignItems: wide ? 'center' : 'stretch', gap: 48 }}>
        {side}
        <View style={{ flex: wide ? 1 : undefined, minWidth: 0, justifyContent: 'center', gap: 32 }}>
          <Stage party={p} unit={Math.max(0.6, unit)} wide={wide} />
        </View>
      </View>
    </ScrollView>
  );
}

const byIdOf = (p: Party) => new Map<number, Member>(p.players.map((m) => [m.id, m]));

/** What the room is looking at: the players while waiting, the question, the result, the end. */
function Stage({ party, unit, wide }: { party: Party; unit: number; wide: boolean }) {
  const t = useTheme();
  const tr = useT();
  const big = (n: number) => Math.round(n * unit);
  if (party.phase === 'voting' && party.round) return <Question key={party.round.id} party={party} round={party.round} unit={unit} />;
  if (party.phase === 'reveal' && party.round) return <Result key={party.round.id} party={party} round={party.round} unit={unit} wide={wide} />;
  if (party.phase === 'over')
    return (
      <Animated.View entering={motion.enter} style={{ gap: 16 }}>
        <Display size={big(96)}>{tr('party.over')}</Display>
        <Text style={{ fontFamily: t.font.semibold, fontSize: big(32), color: t.mut }}>{tr('party.memory.played', { count: party.played })}</Text>
      </Animated.View>
    );
  return (
    <View style={{ gap: 28 }}>
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
function Result({ party, round, unit, wide }: { party: Party; round: PartyRound; unit: number; wide: boolean }) {
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
      {!!round.reactions?.length && <Reactions round={round} disabled />}
    </View>
  );
}
