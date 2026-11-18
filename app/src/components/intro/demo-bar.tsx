import { Crown } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { Mascot } from '@/components/mascot/mascot';
import { Fill, Land, Landed } from '@/components/result/result-bars';
import { useT } from '@/lib/i18n';
import { type DemoRow } from '@/lib/utils/intro';
import { MASCOT } from '@/lib/utils/mascot';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';

export function DemoBar({ row: r, index, rows }: { row: DemoRow; index: number; rows: number }) {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  const color = MASCOT.bodies[r.who].color;
  const name = tr(`intro.names.${r.who}`);
  return (
    <Land
      on={r.win}
      style={[
        { height: 60, borderRadius: 18, overflow: 'hidden', backgroundColor: t.card, justifyContent: 'center' },
        t.shadow,
        r.win && { borderWidth: 2, borderColor: color },
      ]}>
      <Fill index={index} width={r.pct} color={color} opacity={r.win ? 0.5 : 0.25} />
      <View
        accessible
        accessibilityLabel={tr('intro.verdict.votesA11y', { name, count: r.voters.length, pct: r.pct })}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingHorizontal: 10 }}>
        <Mascot who={r.who} mood={r.win ? 'caught' : 'happy'} size={40} />
        <View style={{ flex: 1, flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>
            {name}
          </Text>
          {r.win && (
            <Landed rows={rows}>
              <Crown size={18} color={color} />
            </Landed>
          )}
        </View>
        <View style={{ flexDirection: 'row' }}>
          {r.voters.slice(0, 5).map((v, k) => (
            <View key={v} style={{ marginLeft: k ? -6 : 0, borderRadius: 12, borderWidth: 2, borderColor: t.card }}>
              {v === 'me' ? (
                <Avatar member={me ?? undefined} size={20} />
              ) : (
                <View style={{ width: 20, height: 20, borderRadius: 10, backgroundColor: MASCOT.bodies[v].color }} />
              )}
            </View>
          ))}
        </View>
        <Text style={{ fontFamily: t.font.display, fontSize: 20, color: t.txt, minWidth: 48, textAlign: 'right' }}>{r.pct}%</Text>
      </View>
    </Land>
  );
}
