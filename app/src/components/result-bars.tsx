import { Crown } from 'lucide-react-native';
import { Text, View } from 'react-native';

import type { Member, Tally } from '@/lib/api';
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
