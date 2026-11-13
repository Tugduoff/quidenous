import { BellRing, Handshake, Hourglass, type LucideIcon, MessageCircle, MessageCircleQuestion, Scale, Timer, Vote } from 'lucide-react-native';
import { StyleSheet, Text, View } from 'react-native';

import { Card } from '@/components/ui';
import type { Stats } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

import { duration, int } from './common';

function Tile({ Icon, value, label }: { Icon: LucideIcon; value: string; label: string }) {
  const t = useTheme();
  return (
    <Card style={{ flexBasis: '45%', flexGrow: 1, padding: 16, gap: 6 }}>
      <Icon size={18} color={t.accInk} />
      <Text numberOfLines={1} style={{ fontFamily: t.font.display, fontSize: value.length > 6 ? 26 : 32, letterSpacing: -1, color: t.txt }}>
        {value}
      </Text>
      <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
        {label}
      </Text>
    </Card>
  );
}

type Fact = { Icon: LucideIcon; value: string; label: string };

/** The smaller facts as one list: icon, label, value aligned on the right. */
function Facts({ facts }: { facts: Fact[] }) {
  const t = useTheme();
  return (
    <Card style={{ paddingHorizontal: 16, paddingVertical: 4 }}>
      {facts.map((f, i) => (
        <View
          key={f.label}
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 12,
            height: 44,
            borderTopWidth: i ? StyleSheet.hairlineWidth : 0,
            borderTopColor: t.line,
          }}>
          <f.Icon size={18} color={t.mut} />
          <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.regular, fontSize: 15, color: t.txt }}>
            {f.label}
          </Text>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt, fontVariant: ['tabular-nums'] }}>
            {f.value}
          </Text>
        </View>
      ))}
    </Card>
  );
}

/** The period in four big numbers, and a few smaller facts as a list (only those that happened). */
export function KeyNumbers({ s }: { s: Stats }) {
  const tr = useT();
  const facts = [
    s.unanimous > 0 && { Icon: Handshake, value: int(s.unanimous), label: tr('stats.key.unanimous', { count: s.unanimous }) },
    s.ties > 0 && { Icon: Scale, value: int(s.ties), label: tr('stats.key.ties', { count: s.ties }) },
    s.nudges > 0 && { Icon: BellRing, value: int(s.nudges), label: tr('stats.key.nudges', { count: s.nudges }) },
    s.seconds_waiting_last > 0 && { Icon: Hourglass, value: duration(s.seconds_waiting_last), label: tr('stats.key.waiting') },
    s.median_round_seconds != null && { Icon: Timer, value: duration(s.median_round_seconds), label: tr('stats.key.perQuestion') },
  ].filter((f): f is Fact => !!f);
  return (
    <View style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
        <Tile Icon={MessageCircleQuestion} value={int(s.questions)} label={tr('stats.unit.questions', { count: s.questions })} />
        <Tile Icon={Vote} value={int(s.votes)} label={tr('stats.unit.votes', { count: s.votes })} />
        {s.messages != null && <Tile Icon={MessageCircle} value={int(s.messages)} label={tr('stats.unit.messages', { count: s.messages })} />}
        {s.seconds_voting > 0 && <Tile Icon={Timer} value={duration(s.seconds_voting)} label={tr('stats.key.voting')} />}
      </View>
      {facts.length > 0 && <Facts facts={facts} />}
    </View>
  );
}
