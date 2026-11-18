import { memo } from 'react';
import { ChevronRight } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { until } from '@/lib/utils/time';
import { type TodayRoom } from '@/lib/api/today';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';
import { QuestionText } from '@/components/ui/text';
import { cardQuestion, missing, needsMe, streakAtRisk } from '@/lib/utils/today';
import { StatePill } from '@/components/today/state-pill';
import { StreakChip } from '@/components/today/streak-chip';
import { UnreadChip } from '@/components/today/unread-chip';
import { VotesMeter } from '@/components/today/votes-meter';

export const TodayCard = memo(function TodayCard({ room: r, onOpen }: { room: TodayRoom; onOpen: (id: number) => void }) {
  const t = useTheme();
  const tr = useT();
  const q = cardQuestion(r);
  const mine = needsMe(r);
  const risk = streakAtRisk(r);
  const line = stateLine(r, tr);
  const hint = r.state === 'vote' ? 'today.hint.vote' : r.state === 'result' ? 'today.hint.result' : 'today.hint.other';
  const label = [
    r.name,
    q && q.text,
    line,
    r.streak && r.streak.days > 0 ? tr('streaks.a11y', { count: r.streak.days }) : null,
    risk ? tr('today.atRisk') : null,
    r.unread ? tr('today.unreadA11y', { count: r.unread }) : null,
  ]
    .filter(Boolean)
    .join('. ');
  return (
    <PressableScale scale={0.98} accessibilityRole="button" accessibilityLabel={label} accessibilityHint={tr(hint)} onPress={() => onOpen(r.id)}>
      <Card style={{ padding: 16, gap: 10, borderWidth: 1.5, borderColor: r.state === 'vote' ? t.accInk : r.state === 'result' ? t.tagInk : 'transparent' }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
            {r.name}
          </Text>
          {r.streak && r.streak.days > 0 && <StreakChip days={r.streak.days} risk={risk} />}
          {r.unread > 0 && <UnreadChip count={r.unread} />}
        </View>
        {q ? (
          <QuestionText
            text={q.text}
            rank={q.kind === 'rank'}
            choose={q.kind === 'choose'}
            style={{ fontFamily: t.font.bold, fontSize: 18, lineHeight: 24, color: r.state === 'idle' ? t.mut : t.txt }}
          />
        ) : null}
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, marginTop: 2 }}>
          <StatePill room={r} line={line} />
          <View style={{ flex: 1 }} />
          {r.state === 'vote' || r.state === 'wait' || r.state === 'tripot' ? <VotesMeter votes={r.votes} needed={r.needed} /> : null}
          {!mine && <ChevronRight size={18} color={t.mut} />}
        </View>
      </Card>
    </PressableScale>
  );
});

function stateLine(r: TodayRoom, tr: ReturnType<typeof useT>) {
  switch (r.state) {
    case 'vote':
      return tr('today.state.vote');
    case 'result':
      return tr('today.state.result', { count: Math.max(1, r.unseen) });
    case 'tripot':
      return tr('today.state.tripot');
    case 'wait': {
      const n = missing(r);
      return n ? tr('today.state.wait', { count: n }) : tr('today.state.waitAll');
    }
    default:
      return r.next_open_at ? tr('today.state.next', { time: until(r.next_open_at) }) : tr('today.state.none');
  }
}
