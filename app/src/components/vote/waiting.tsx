import { Eye, EyeOff } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { myRealChoice } from '@/components/choose/choose';
import { Screen } from '@/components/ui/screen';
import { BetRow } from '@/components/tripot/tripot';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { setHideVote, useHideVote } from '@/lib/device/prefs';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { type Member } from '@/lib/api/group';
import { type Round } from '@/lib/api/rounds';
import { Question } from '@/components/vote/question';
import { Header } from '@/components/vote/header';
import { WhoIsIn } from '@/components/vote/who-is-in';

export function Waiting({ round, byId, members }: { round: Round; byId: Map<number, Member>; members: Member[] }) {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  // A choose question: my own real choice (my vote's target is me).
  const choice = round.kind === 'choose' ? myRealChoice(round, me?.id) : null;
  const target = round.my_vote != null ? byId.get(round.my_vote) : undefined;
  const hidden = useHideVote();
  const done = round.voter_ids.length;
  const label = { fontFamily: t.font.bold, fontSize: 11, letterSpacing: 0.6, color: t.mut } as const;
  return (
    <Screen>
      <Header />
      <Question round={round} />
      {/* My vote: who I picked (or hidden), one switch to hide it when friends can see my screen. */}
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14 }}>
        {hidden ? (
          <View style={{ width: 44, height: 44, borderRadius: 22, backgroundColor: t.card2, alignItems: 'center', justifyContent: 'center' }}>
            <EyeOff size={20} color={t.mut} />
          </View>
        ) : (
          <Avatar member={target} size={44} />
        )}
        <View style={{ flex: 1, gap: 2 }}>
          <Text style={label}>
            {tr(round.kind === 'rank' ? 'vote.waiting.yourFirst' : round.kind === 'choose' ? 'vote.waiting.yourChoice' : 'vote.waiting.yourVote')}
          </Text>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 17, color: hidden ? t.mut : t.txt }}>
            {hidden ? tr('vote.waiting.hidden') : round.kind === 'choose' ? (choice ?? '?') : (target?.name ?? '?')}
          </Text>
        </View>
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr(hidden ? 'vote.waiting.show' : 'vote.waiting.hide')}
          onPress={() => setHideVote(!hidden)}
          hitSlop={8}
          style={{ width: 36, height: 36, borderRadius: 18, backgroundColor: t.card2, alignItems: 'center', justifyContent: 'center' }}>
          {hidden ? <Eye size={18} color={t.txt} /> : <EyeOff size={18} color={t.txt} />}
        </PressableScale>
      </Card>
      <BetRow round={round} members={members} byId={byId} />
      <WhoIsIn
        round={round}
        members={members}
        byId={byId}
        done={round.voter_ids}
        title={tr('vote.waiting.voted', { count: done, done, total: round.member_count })}
      />
    </Screen>
  );
}
