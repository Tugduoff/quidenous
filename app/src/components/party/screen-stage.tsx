import { useQueryClient } from '@tanstack/react-query';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { MascotBand } from '@/components/mascot/band';
import { Mascot } from '@/components/mascot/mascot';
import { Countdown, PlayerChips, Reactions, VotedLine, WinnerReveal } from '@/components/party/party';
import { SaidScreenQuestion, SaidScreenResult } from '@/components/party/said-screen';
import { ScreenCounts } from '@/components/result/live-reactions';
import { ResultBars } from '@/components/result/result-bars';
import { Tag } from '@/components/ui/tag';
import { Display } from '@/components/ui/text';
import { type Party, partyKeys, type PartyRound } from '@/lib/api/party';
import { useCountdown } from '@/lib/hooks/use-countdown';
import { useRefetchAtZero } from '@/lib/hooks/use-refetch-at-zero';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { playersById, tallies } from '@/lib/utils/party';
import { tidy } from '@/lib/utils/text';

/** What the room is looking at: the players while waiting, the question, the result, the end. */
export function ScreenStage({ party, unit, wide, reactions }: { party: Party; unit: number; wide: boolean; reactions: boolean }) {
  const t = useTheme();
  const tr = useT();
  const big = (n: number) => Math.round(n * unit);
  const said = party.round?.kind === 'said' ? party.round.said : null;
  if (party.phase === 'voting' && party.round && said)
    return <SaidScreenQuestion key={`${party.round.id}-${said.phase}`} party={party} round={party.round} said={said} unit={unit} wide={wide} />;
  if (party.phase === 'reveal' && party.round && said)
    return <SaidScreenResult key={party.round.id} party={party} round={party.round} said={said} unit={unit} wide={wide} reactions={reactions} />;
  if (party.phase === 'voting' && party.round) return <ScreenQuestion key={party.round.id} party={party} round={party.round} unit={unit} />;
  if (party.phase === 'reveal' && party.round) return <ScreenResult key={party.round.id} party={party} round={party.round} unit={unit} wide={wide} reactions={reactions} />;
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

function ScreenQuestion({ party, round, unit }: { party: Party; round: PartyRound; unit: number }) {
  const ms = useCountdown(round.deadline_at, party.clock);
  const qc = useQueryClient();
  useRefetchAtZero(ms, () => qc.invalidateQueries({ queryKey: partyKeys.screen(party.code) }));
  const size = Math.round((round.question.length > 90 ? 64 : 84) * unit);
  return (
    <Animated.View entering={motion.question} style={{ gap: 36 * unit }}>
      <Tag label={`#${round.number}`} />
      <Display size={size}>{tidy(round.question)}</Display>
      <Countdown ms={ms} total={party.timer} big />
      <VotedLine round={round} byId={playersById(party)} big />
    </Animated.View>
  );
}

/** The result: on a wide screen the winner and the bars side by side, so it all fits without scrolling. */
function ScreenResult({ party, round, unit, wide, reactions }: { party: Party; round: PartyRound; unit: number; wide: boolean; reactions: boolean }) {
  const t = useTheme();
  const byId = playersById(party);
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
