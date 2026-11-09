import {
  AlarmClock,
  BellRing,
  Crown,
  Feather,
  Flower2,
  Ghost,
  Heart,
  Hourglass,
  Snail,
  Star,
  Zap,
} from 'lucide-react-native';
import type { ComponentType } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, AvatarStack, Card, Display } from '@/components/ui';
import type { Award, Member, Stats } from '@/lib/api';
import { useMembers, useStats } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

type Icon = ComponentType<{ size?: number; color?: string }>;

/** "45 s", "12 min", "3 h 20", "2 j 4 h". */
function duration(seconds: number) {
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return `${s} s`;
  const min = Math.round(s / 60);
  if (min < 60) return `${min} min`;
  const h = Math.floor(s / 3600);
  if (h < 24) {
    const m = Math.round((s % 3600) / 60);
    return m ? `${h} h ${String(m).padStart(2, '0')}` : `${h} h`;
  }
  const d = Math.floor(h / 24);
  return h % 24 ? `${d} j ${h % 24} h` : `${d} j`;
}

const plural = (n: number, one: string, many: string) => `${n} ${n > 1 ? many : one}`;

function Tile({ value, label }: { value: string; label: string }) {
  const t = useTheme();
  return (
    <Card style={{ flex: 1, padding: 14, gap: 2 }}>
      <Text style={{ fontFamily: t.font.display, fontSize: 24, letterSpacing: -0.5, color: t.txt }} numberOfLines={1}>
        {value}
      </Text>
      <Text style={{ fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: t.mut }}>{label}</Text>
    </Card>
  );
}

function AwardCard({
  Icon,
  title,
  members,
  caption,
}: {
  Icon: Icon;
  title: string;
  members: (Member | undefined)[];
  caption: string;
}) {
  const t = useTheme();
  const tint = members.length === 1 ? (members[0]?.color ?? t.accInk) : t.accInk;
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14 }}>
      {members.length === 1 ? (
        <Avatar member={members[0]} size={44} />
      ) : (
        <AvatarStack members={members} size={36} ring={t.card} max={3} />
      )}
      <View style={{ flex: 1, gap: 2 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          <Icon size={14} color={tint} />
          <Text style={{ fontFamily: t.font.bold, fontSize: 12, letterSpacing: 0.3, color: t.mut, textTransform: 'uppercase' }}>
            {title}
          </Text>
        </View>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}>{caption}</Text>
      </View>
    </Card>
  );
}

function Fact({ children }: { children: string }) {
  const t = useTheme();
  return (
    <Text style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 20, color: t.txt }}>
      <Text style={{ color: t.accInk }}>●  </Text>
      {children}
    </Text>
  );
}

/** Each award: the stat it reads, how to say it. Hidden when nobody qualifies yet. */
const AWARDS: { key: keyof Stats; Icon: Icon; title: string; caption: (name: string, v: number) => string }[] = [
  { key: 'most_chosen', Icon: Star, title: 'La star', caption: (n, v) => `${n}, ${plural(v, 'vote reçu', 'votes reçus')}` },
  { key: 'most_crowned', Icon: Crown, title: 'Collection de couronnes', caption: (n, v) => `${n}, ${plural(v, 'question gagnée', 'questions gagnées')}` },
  { key: 'narcissist', Icon: Flower2, title: 'Narcisse', caption: (n, v) => `${n} a voté ${plural(v, 'fois', 'fois')} pour soi` },
  { key: 'fastest', Icon: Zap, title: 'Gâchette facile', caption: (n, v) => `${n} vote en ${duration(v)} en général` },
  { key: 'slowest', Icon: Snail, title: 'Prend son temps', caption: (n, v) => `${n} met ${duration(v)} à voter en général` },
  { key: 'last_voter', Icon: Hourglass, title: 'Tout le monde attend', caption: (n, v) => `${n}, ${plural(v, 'fois', 'fois')} le dernier vote` },
  { key: 'nudger', Icon: BellRing, title: 'Relou officiel', caption: (n, v) => `${n}, ${plural(v, 'relance envoyée', 'relances envoyées')}` },
  { key: 'most_nudged', Icon: AlarmClock, title: 'Toujours relancé', caption: (n, v) => `${n}, ${plural(v, 'relance reçue', 'relances reçues')}` },
  { key: 'author', Icon: Feather, title: 'La plume', caption: (n, v) => `${n}, ${plural(v, 'question proposée', 'questions proposées')}` },
];

export default function StatsScreen() {
  const t = useTheme();
  const { data: s, isLoading } = useStats();
  const { byId } = useMembers();
  const name = (id: number) => byId.get(id)?.name ?? '?';

  if (isLoading || !s) {
    return (
      <Screen>
        <Display size={30}>Stats</Display>
        {isLoading && <ActivityIndicator color={t.mut} />}
      </Screen>
    );
  }
  if (s.questions === 0) {
    return (
      <Screen>
        <Display size={30}>Stats</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>
          Rien à compter pour l'instant : finissez une première question.
        </Text>
      </Screen>
    );
  }

  const facts = [
    s.seconds_waiting_last > 0 && `${duration(s.seconds_waiting_last)} passées à attendre le dernier vote`,
    s.median_round_seconds != null && `Une question dure ${duration(s.median_round_seconds)} en général`,
    s.unanimous > 0 && `${plural(s.unanimous, 'question', 'questions')} à l'unanimité`,
    s.ties > 0 && `${plural(s.ties, 'égalité', 'égalités')}`,
    s.nudges > 0 && `${plural(s.nudges, 'relance', 'relances')} au total`,
  ].filter((f): f is string => !!f);

  return (
    <Screen>
      <Display size={30}>Stats</Display>
      <View style={{ flexDirection: 'row', gap: 10 }}>
        <Tile value={String(s.questions)} label={s.questions > 1 ? 'questions' : 'question'} />
        <Tile value={String(s.votes)} label={s.votes > 1 ? 'votes' : 'vote'} />
        <Tile value={duration(s.seconds_voting)} label="perdues à voter (à la louche)" />
      </View>

      <View style={{ gap: 10 }}>
        {AWARDS.map(({ key, Icon, title, caption }) => {
          const a = s[key] as Award | null;
          return a ? (
            <AwardCard key={key} Icon={Icon} title={title} members={[byId.get(a.member_id)]} caption={caption(name(a.member_id), a.value)} />
          ) : null;
        })}
        {s.duo && (
          <AwardCard
            Icon={Heart}
            title="Le crush"
            members={[byId.get(s.duo.voter_id), byId.get(s.duo.target_id)]}
            caption={`${name(s.duo.voter_id)} a voté ${s.duo.value} fois pour ${name(s.duo.target_id)}`}
          />
        )}
        {s.never_chosen.length > 0 && (
          <AwardCard
            Icon={Ghost}
            title="Les invisibles"
            members={s.never_chosen.map((id) => byId.get(id))}
            caption={`${s.never_chosen.map(name).join(', ')} : zéro vote reçu`}
          />
        )}
      </View>

      {facts.length > 0 && (
        <Card style={{ padding: 16, gap: 8 }}>
          {facts.map((f) => (
            <Fact key={f}>{f}</Fact>
          ))}
        </Card>
      )}
    </Screen>
  );
}
