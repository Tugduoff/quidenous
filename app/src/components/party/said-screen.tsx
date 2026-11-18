import { useQueryClient } from '@tanstack/react-query';
import { memo, useMemo } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Countdown, Reactions, VotedLine } from '@/components/party/party';
import { SaidTooFew } from '@/components/party/said-too-few';
import { ScreenCounts } from '@/components/result/live-reactions';
import { AnswerText } from '@/components/said/answer-text';
import { NumberDisc } from '@/components/said/number-disc';
import { SaidScoreboard } from '@/components/said/said-scoreboard';
import { SaidScreenReveal } from '@/components/said/said-screen-reveal';
import { Tag } from '@/components/ui/tag';
import { Display } from '@/components/ui/text';
import type { Member } from '@/lib/api/group';
import { type Party, partyKeys, type PartyRound } from '@/lib/api/party';
import type { SaidAnswer, SaidView } from '@/lib/api/rounds';
import { useCountdown } from '@/lib/hooks/use-countdown';
import { useScreenStep } from '@/lib/hooks/use-party';
import { usePartyPlayers } from '@/lib/hooks/use-party-players';
import { useRefetchAtZero } from '@/lib/hooks/use-refetch-at-zero';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { phaseSeconds } from '@/lib/utils/party';
import { revealItems, screenRevealSteps, toGuess } from '@/lib/utils/said';
import { tidy } from '@/lib/utils/text';

function KindTag({ round }: { round: PartyRound }) {
  const tr = useT();
  return <Tag label={`#${round.number} · ${tr('said.reveal.title')}`} />;
}

export function SaidScreenQuestion({ party, round, said, unit, wide }: { party: Party; round: PartyRound; said: SaidView; unit: number; wide: boolean }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const ms = useCountdown(round.deadline_at, party.clock);
  useRefetchAtZero(ms, () => qc.invalidateQueries({ queryKey: partyKeys.screen(party.code) }));
  const byId = usePartyPlayers(party);
  const total = phaseSeconds(round, party.timer);
  const big = (n: number) => Math.round(n * unit);
  if (said.phase === 'writing') {
    const writers = said.writer_ids.filter((id) => round.eligible.includes(id));
    return (
      <Animated.View entering={motion.question} style={{ gap: 32 * unit }}>
        <KindTag round={round} />
        <Display size={big(round.question.length > 90 ? 64 : 84)}>{tidy(round.question)}</Display>
        <Text style={{ fontFamily: t.font.semibold, fontSize: big(30), lineHeight: big(40), color: t.mut }}>{tr('said.party.write')}</Text>
        <Countdown ms={ms} total={total} big />
        <VotedLine round={round} byId={byId} done={writers} label="said.party.written" big />
      </Animated.View>
    );
  }
  return <ScreenAnswers round={round} said={said} byId={byId} ms={ms} total={total} unit={unit} wide={wide} />;
}

function ScreenAnswers({
  round,
  said,
  byId,
  ms,
  total,
  unit,
  wide,
}: {
  round: PartyRound;
  said: SaidView;
  byId: Map<number, Member>;
  ms: number | null;
  total: number;
  unit: number;
  wide: boolean;
}) {
  const t = useTheme();
  const tr = useT();
  const answers = useMemo(() => toGuess(said), [said]);
  const cols = wide && answers.length > 3 ? 2 : 1;
  const half = Math.ceil(answers.length / 2);
  const size = Math.round((answers.length > 6 ? 28 : answers.length > 3 ? 34 : 42) * unit);
  return (
    <Animated.View entering={motion.question} style={{ gap: 28 * unit }}>
      <View style={{ gap: 14 * unit }}>
        <KindTag round={round} />
        <Text numberOfLines={2} style={{ fontFamily: t.font.bold, fontSize: Math.round(36 * unit), lineHeight: Math.round(44 * unit), color: t.txt }}>
          {tidy(round.question)}
        </Text>
      </View>
      <View style={{ flexDirection: 'row', gap: 16 * unit }}>
        {(cols === 2 ? [answers.slice(0, half), answers.slice(half)] : [answers]).map((col, c) => (
          <View key={c} style={{ flex: 1, gap: 16 * unit }}>
            {col.map((a, i) => {
              const n = c * half + i + 1;
              return <ScreenAnswer key={a.id} answer={a} n={n} total={answers.length} size={size} />;
            })}
          </View>
        ))}
      </View>
      <View style={{ flexDirection: wide ? 'row' : 'column', alignItems: wide ? 'center' : 'stretch', gap: 32 * unit }}>
        <View style={{ flex: wide ? 1 : undefined, gap: 10 * unit }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: Math.round(24 * unit), color: t.mut }}>{tr('said.party.guess')}</Text>
          <VotedLine round={round} byId={byId} label="said.party.guessed" big />
        </View>
        <View style={{ flex: wide ? 1 : undefined }}>
          <Countdown ms={ms} total={total} big />
        </View>
      </View>
    </Animated.View>
  );
}

const ScreenAnswer = memo(function ScreenAnswer({
  answer,
  n,
  total,
  size,
}: {
  answer: SaidAnswer;
  n: number;
  total: number;
  size: number;
}) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View
      entering={motion.stagger(n - 1)}
      accessible
      accessibilityLabel={tr('said.guess.a11y', { n, total, text: answer.text ?? tr('said.guess.hidden') })}
      style={[
        {
          flexDirection: 'row',
          alignItems: 'flex-start',
          gap: Math.round(size * 0.6),
          padding: Math.round(size * 0.75),
          borderRadius: t.radius.card,
          backgroundColor: t.card,
        },
        t.shadow,
      ]}>
      <NumberDisc n={n} size={Math.round(size * 1.5)} />
      <View style={{ flex: 1, paddingTop: Math.round(size * 0.12) }}>
        <AnswerText answer={answer} size={size} />
      </View>
    </Animated.View>
  );
});

export function SaidScreenResult({
  party,
  round,
  said,
  unit,
  wide,
  reactions,
}: {
  party: Party;
  round: PartyRound;
  said: SaidView;
  unit: number;
  wide: boolean;
  reactions: boolean;
}) {
  const t = useTheme();
  const byId = usePartyPlayers(party);
  const items = useMemo(() => revealItems(said, undefined), [said]);
  const steps = useMemo(() => screenRevealSteps(items), [items]);
  const step = useScreenStep(round.closed_at, party.clock, steps);
  const result = said.result;
  return (
    <View style={{ gap: 32 * unit }}>
      <View style={{ gap: 14 * unit }}>
        <KindTag round={round} />
        <Text numberOfLines={2} style={{ fontFamily: t.font.bold, fontSize: Math.round(34 * unit), lineHeight: Math.round(42 * unit), color: t.txt }}>
          {tidy(round.question)}
        </Text>
      </View>
      {!result ? (
        <SaidTooFew size={Math.round(160 * unit)} big />
      ) : step < items.length ? (
        <SaidScreenReveal items={items} step={step} byId={byId} k={unit} wide={wide} />
      ) : (
        <Animated.View entering={motion.enter} style={{ gap: 28 * unit }}>
          <SaidScoreboard scores={result.scores} byId={byId} me={undefined} k={Math.max(1, 1.8 * unit)} columns={wide && result.scores.length > 4 ? 2 : 1} />
          {reactions ? <ScreenCounts reactions={round.reactions} unit={unit} /> : !!round.reactions?.length && <Reactions round={round} disabled />}
        </Animated.View>
      )}
    </View>
  );
}
