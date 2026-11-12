import { Crown } from 'lucide-react-native';
import { Text, View } from 'react-native';

import type { Member, RankEntry, Tally } from '@/lib/api';
import { useTheme } from '@/lib/theme';

import { Avatar, AvatarStack } from './ui';

export function ResultBars({ result, byId }: { result: Tally[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const total = result.reduce((n, r) => n + r.count, 0) || 1;
  const top = result[0]?.count ?? 0;
  return (
    <View style={{ gap: 12 }}>
      {result.map((r) => {
        const m = byId.get(r.target_id);
        const color = m?.color ?? t.mut;
        const win = r.count === top;
        const pct = Math.round((r.count / total) * 100);
        return (
          <View
            key={r.target_id}
            style={[
              { height: 64, borderRadius: 18, overflow: 'hidden', backgroundColor: t.card, justifyContent: 'center' },
              t.shadow,
              win && { borderWidth: 2, borderColor: color },
            ]}>
            <View
              style={{
                position: 'absolute',
                left: 0,
                top: 0,
                bottom: 0,
                width: `${pct}%`,
                backgroundColor: color,
                opacity: win ? 0.55 : 0.28,
              }}
            />
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingHorizontal: 12 }}>
              <Avatar member={m} />
              <View style={{ flex: 1, flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }} numberOfLines={1}>
                  {m?.name}
                </Text>
                {win && <Crown size={18} color={color} />}
              </View>
              <AvatarStack members={r.voter_ids.map((id) => byId.get(id))} size={24} />
              <Text style={{ fontFamily: t.font.display, fontSize: 22, color: t.txt }}>{pct}%</Text>
            </View>
          </View>
        );
      })}
    </View>
  );
}

/** Ranking result: everyone in order, with points (Borda) and how many put them first. */
export function RankResult({ ranking, byId, mine }: { ranking: RankEntry[]; byId: Map<number, Member>; mine: number[] | null }) {
  const t = useTheme();
  const best = ranking[0]?.points || 1;
  // Gold, silver, bronze discs for the podium.
  const podium = ['#F5C542', '#C9D0DA', '#D9935A'];
  return (
    <View style={{ gap: 10 }}>
      {ranking.map((r, i) => {
        const m = byId.get(r.member_id);
        const color = m?.color ?? t.mut;
        const myPlace = mine ? mine.indexOf(r.member_id) + 1 : 0;
        return (
          <View
            key={r.member_id}
            style={[
              { height: 60, borderRadius: 18, overflow: 'hidden', backgroundColor: t.card, justifyContent: 'center' },
              t.shadow,
              i === 0 && { borderWidth: 2, borderColor: color },
            ]}>
            <View
              style={{
                position: 'absolute',
                left: 0,
                top: 0,
                bottom: 0,
                width: `${Math.round((r.points / best) * 100)}%`,
                backgroundColor: color,
                opacity: i === 0 ? 0.5 : 0.25,
              }}
            />
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingHorizontal: 12 }}>
              <View
                style={{
                  width: 30,
                  height: 30,
                  borderRadius: 15,
                  alignItems: 'center',
                  justifyContent: 'center',
                  backgroundColor: podium[i] ?? 'transparent',
                }}>
                <Text style={{ fontFamily: t.font.display, fontSize: i < 3 ? 15 : 18, color: i < 3 ? '#1A1A1A' : t.txt }}>{i + 1}</Text>
              </View>
              <Avatar member={m} size={36} />
              <View style={{ flex: 1 }}>
                <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }} numberOfLines={1}>
                  {m?.name}
                </Text>
                <Text style={{ fontFamily: t.font.regular, fontSize: 11, color: t.mut }}>
                  {r.firsts ? `1er pour ${r.firsts} · ` : ''}moyenne {r.average.toFixed(1).replace('.', ',')}
                  {myPlace ? ` · toi : ${myPlace}e` : ''}
                </Text>
              </View>
              <Text style={{ fontFamily: t.font.display, fontSize: 20, color: t.txt }}>{r.points}</Text>
            </View>
          </View>
        );
      })}
      <Text style={{ fontFamily: t.font.regular, fontSize: 11, color: t.mut, marginLeft: 4 }}>
        Points : chaque classement donne {ranking.length} points au 1er, 1 au dernier.
      </Text>
    </View>
  );
}
