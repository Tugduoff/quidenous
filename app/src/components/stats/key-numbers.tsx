import { BellRing, Handshake, Hourglass, type LucideIcon, MessageCircle, MessageCircleQuestion, Scale, Timer, Vote } from 'lucide-react-native';
import { StyleSheet, Text, View } from 'react-native';

import { Card } from '@/components/ui';
import type { Stats } from '@/lib/api';
import { useTheme } from '@/lib/theme';

import { duration, int, plural } from './common';

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
  const facts = [
    s.unanimous > 0 && { Icon: Handshake, value: int(s.unanimous), label: plural(s.unanimous, 'Unanime', 'Unanimes') },
    s.ties > 0 && { Icon: Scale, value: int(s.ties), label: plural(s.ties, 'Égalité', 'Égalités') },
    s.nudges > 0 && { Icon: BellRing, value: int(s.nudges), label: plural(s.nudges, 'Relance', 'Relances') },
    s.seconds_waiting_last > 0 && { Icon: Hourglass, value: duration(s.seconds_waiting_last), label: 'À attendre' },
    s.median_round_seconds != null && { Icon: Timer, value: duration(s.median_round_seconds), label: 'Par question' },
  ].filter((f): f is Fact => !!f);
  return (
    <View style={{ gap: 12 }}>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
        <Tile Icon={MessageCircleQuestion} value={int(s.questions)} label={plural(s.questions, 'question', 'questions')} />
        <Tile Icon={Vote} value={int(s.votes)} label={plural(s.votes, 'vote', 'votes')} />
        {s.messages != null && <Tile Icon={MessageCircle} value={int(s.messages)} label={plural(s.messages, 'message', 'messages')} />}
        {s.seconds_voting > 0 && <Tile Icon={Timer} value={duration(s.seconds_voting)} label="à voter" />}
      </View>
      {facts.length > 0 && <Facts facts={facts} />}
    </View>
  );
}
