import { BellRing, Handshake, Hourglass, type LucideIcon, MessageCircle, MessageCircleQuestion, Scale, Timer, Vote } from 'lucide-react-native';
import { Text, View } from 'react-native';

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

function Fact({ Icon, children }: { Icon: LucideIcon; children: string }) {
  const t = useTheme();
  return (
    <View
      style={{
        flexDirection: 'row',
        alignItems: 'center',
        gap: 6,
        paddingHorizontal: 12,
        height: 32,
        borderRadius: t.radius.pill,
        backgroundColor: t.card2,
      }}>
      <Icon size={14} color={t.mut} />
      <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.txt }}>
        {children}
      </Text>
    </View>
  );
}

/** The period in four big numbers, and a few smaller facts as chips (only those that happened). */
export function KeyNumbers({ s }: { s: Stats }) {
  const facts = [
    s.unanimous > 0 && { Icon: Handshake, text: `${s.unanimous} ${plural(s.unanimous, 'unanime', 'unanimes')}` },
    s.ties > 0 && { Icon: Scale, text: `${s.ties} ${plural(s.ties, 'égalité', 'égalités')}` },
    s.nudges > 0 && { Icon: BellRing, text: `${s.nudges} ${plural(s.nudges, 'relance', 'relances')}` },
    s.seconds_waiting_last > 0 && { Icon: Hourglass, text: `${duration(s.seconds_waiting_last)} à attendre` },
    s.median_round_seconds != null && { Icon: Timer, text: `${duration(s.median_round_seconds)} par question` },
  ].filter((f): f is { Icon: LucideIcon; text: string } => !!f);
  return (
    <View style={{ gap: 12 }}>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
        <Tile Icon={MessageCircleQuestion} value={int(s.questions)} label={plural(s.questions, 'question', 'questions')} />
        <Tile Icon={Vote} value={int(s.votes)} label={plural(s.votes, 'vote', 'votes')} />
        {s.messages != null && <Tile Icon={MessageCircle} value={int(s.messages)} label={plural(s.messages, 'message', 'messages')} />}
        {s.seconds_voting > 0 && <Tile Icon={Timer} value={duration(s.seconds_voting)} label="à voter" />}
      </View>
      {facts.length > 0 && (
        <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8 }}>
          {facts.map((f) => (
            <Fact key={f.text} Icon={f.Icon}>
              {f.text}
            </Fact>
          ))}
        </View>
      )}
    </View>
  );
}
