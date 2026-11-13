import { router } from 'expo-router';
import { ChevronRight } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { Avatar, AvatarStack, PressableScale, QuestionText } from '@/components/ui';
import type { Award, Duo, Member, Pair, QuestionRef, RankAward, Stats } from '@/lib/api';
import { t, type TKey, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

import { AWARD_ICONS, awardTitle, Caps, duration, iconFor, int, pct, place, Section } from './common';

/** One award ready to show: who, the figure, and a quip. */
type Item = { key: string; ids: number[]; who: string; big: string; unit: string; quip: string };
type Name = (id: number) => string;
type UnitKey = 'votes' | 'wins' | 'nightVotes' | 'messages' | 'nudges' | 'questions';

const n = (v: number, unit: UnitKey) => ({ big: int(v), unit: t(`stats.unit.${unit}`, { count: v }) });
const quip = (key: string) => t(`stats.quip.${key}` as TKey);

/** Single-person awards: the figure and its unit, from its value (and total, for ratios). */
const SOLO: Record<string, (a: Award) => { big: string; unit: string }> = {
  most_chosen: (a) => n(a.value, 'votes'),
  most_crowned: (a) => n(a.value, 'wins'),
  streak: (a) => ({ big: int(a.value), unit: t('stats.unit.inARow') }),
  longest_streak: (a) => ({ big: int(a.value), unit: t('stats.unit.straight') }),
  comeback: (a) => ({ big: int(a.value), unit: t('stats.unit.drought') }),
  narcissist: (a) => ({ big: int(a.value), unit: t('stats.unit.forSelf') }),
  contrarian: (a) => ({ big: pct(a.value, a.total), unit: t('stats.unit.againstCrowd') }),
  sheep: (a) => ({ big: pct(a.value, a.total), unit: t('stats.unit.withCrowd') }),
  kingmaker: (a) => ({ big: int(a.value), unit: t('stats.unit.byOneVote') }),
  fastest: (a) => ({ big: duration(a.value), unit: t('stats.unit.perVote') }),
  slowest: (a) => ({ big: duration(a.value), unit: t('stats.unit.perVote') }),
  last_voter: (a) => ({ big: int(a.value), unit: t('stats.unit.timesLast') }),
  night_owl: (a) => n(a.value, 'nightVotes'),
  early_bird: (a) => ({ big: int(a.value), unit: t('stats.unit.before9') }),
  chatterbox: (a) => n(a.value, 'messages'),
  silent: (a) => n(a.value, 'messages'),
  nudger: (a) => n(a.value, 'nudges'),
  most_nudged: (a) => n(a.value, 'nudges'),
  author: (a) => n(a.value, 'questions'),
};

const rank = (a: RankAward) => ({ big: place(a.avg), unit: t('stats.unit.onAverage') });

type GroupKey = 'titles' | 'votes' | 'chat' | 'questions';

/**
 * The award cards, in titled groups; an award nobody holds this period is left out, and so is an empty group.
 * Call at render: the texts are in the current language.
 */
export function awardGroups(s: Stats, name: Name) {
  const solo = (key: string): Item | null => {
    const a = s[key as keyof Stats] as Award | null;
    return a ? { key, ids: [a.member_id], who: name(a.member_id), ...SOLO[key](a), quip: quip(key) } : null;
  };
  const duo = (key: 'duo' | 'loyal', d: Duo | null, unit: (d: Duo) => [string, string]): Item | null =>
    d
      ? {
          key,
          ids: [d.voter_id, d.target_id],
          who: t('stats.forWhom', { a: name(d.voter_id), b: name(d.target_id) }),
          big: unit(d)[0],
          unit: unit(d)[1],
          quip: quip(key),
        }
      : null;
  const pair = (p: Pair | null): Item | null =>
    p
      ? {
          key: 'twins',
          ids: [p.a_id, p.b_id],
          who: t('stats.pair', { a: name(p.a_id), b: name(p.b_id) }),
          big: `${p.value}/${p.total}`,
          unit: t('stats.unit.sameVotes'),
          quip: quip('twins'),
        }
      : null;
  const ranked = (key: 'best_rank' | 'worst_rank'): Item | null => {
    const a = s[key];
    return a ? { key, ids: [a.member_id], who: name(a.member_id), ...rank(a), quip: quip(key) } : null;
  };
  const ghosts: Item | null = s.never_chosen.length
    ? {
        key: 'never_chosen',
        ids: s.never_chosen,
        who: s.never_chosen.map(name).join(', '),
        big: '0',
        unit: t('stats.unit.votes', { count: 0 }),
        quip: quip('never_chosen'),
      }
    : null;
  const groups: { key: GroupKey; items: (Item | null)[] }[] = [
    {
      key: 'titles',
      items: [
        solo('most_chosen'),
        solo('most_crowned'),
        solo('streak'),
        solo('longest_streak'),
        solo('comeback'),
        ranked('best_rank'),
        ranked('worst_rank'),
        ghosts,
      ],
    },
    {
      key: 'votes',
      items: [
        duo('duo', s.duo, (d) => [int(d.value), t('stats.unit.votes', { count: d.value })]),
        duo('loyal', s.loyal, (d) => [pct(d.value, d.total), t('stats.unit.ofVotes')]),
        pair(s.twins),
        solo('contrarian'),
        solo('sheep'),
        solo('kingmaker'),
        solo('narcissist'),
        solo('fastest'),
        solo('slowest'),
        solo('last_voter'),
        solo('night_owl'),
        solo('early_bird'),
      ],
    },
    { key: 'chat', items: [solo('chatterbox'), solo('silent'), solo('nudger'), solo('most_nudged')] },
    { key: 'questions', items: [solo('author')] },
  ];
  return groups.map((g) => ({ key: g.key, title: t(`stats.group.${g.key}`), items: g.items.filter((i): i is Item => !!i) }));
}

function AwardCard({ item, byId, onOpen, index }: { item: Item; byId: Map<number, Member>; onOpen: (id: number) => void; index: number }) {
  const t = useTheme();
  useT();
  const title = awardTitle(item.key);
  const Icon = iconFor(AWARD_ICONS[item.key]);
  const people = item.ids.map((id) => byId.get(id));
  const tint = people.length === 1 ? (people[0]?.color ?? t.accInk) : t.accInk;
  return (
    <Animated.View entering={motion.stagger(index)}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        accessibilityLabel={`${title}: ${item.who}, ${item.big} ${item.unit}`}
        onPress={() => onOpen(item.ids[0])}
        style={[
          { flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14, borderRadius: t.radius.card, backgroundColor: t.card },
          t.shadow,
        ]}>
        {people.length === 1 ? <Avatar member={people[0]} size={44} /> : <AvatarStack members={people} size={34} ring={t.card} max={3} />}
        <View style={{ flex: 1, gap: 3 }}>
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
            <Icon size={13} color={tint} strokeWidth={2.5} />
            <Caps size={11}>{title}</Caps>
          </View>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>
            {item.who}
          </Text>
          <Text numberOfLines={2} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 17, color: t.mut }}>
            {item.quip}
          </Text>
        </View>
        <View style={{ alignItems: 'flex-end', maxWidth: 96 }}>
          <Text numberOfLines={1} style={{ fontFamily: t.font.display, fontSize: item.big.length > 5 ? 20 : 26, letterSpacing: -0.5, color: t.txt }}>
            {item.big}
          </Text>
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 11, color: t.mut }}>
            {item.unit}
          </Text>
        </View>
      </PressableScale>
    </Animated.View>
  );
}

const QUESTION_LINE: Record<string, (q: QuestionRef) => string> = {
  most_split: (q) => t('stats.quip.most_split', { value: q.value, total: q.total }),
  most_unanimous: (q) => t('stats.quip.most_unanimous', { total: q.total }),
  hottest: (q) => t('stats.quip.hottest', { count: q.value }),
};

/** A question award: its title, the question, one line; opens the question. */
function QuestionCard({ k, q, index }: { k: string; q: QuestionRef; index: number }) {
  const t = useTheme();
  useT();
  const Icon = iconFor(AWARD_ICONS[k]);
  return (
    <Animated.View entering={motion.stagger(index)}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        onPress={() => router.push(`/round/${q.round_id}`)}
        style={[{ padding: 16, gap: 8, borderRadius: t.radius.card, backgroundColor: t.card }, t.shadow]}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          <Icon size={13} color={t.accInk} strokeWidth={2.5} />
          <View style={{ flex: 1 }}>
            <Caps size={11}>{awardTitle(k)}</Caps>
          </View>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>#{q.number}</Text>
          <ChevronRight size={16} color={t.mut} />
        </View>
        <QuestionText text={q.question} rank={q.kind === 'rank'} style={{ fontFamily: t.font.bold, fontSize: 17, lineHeight: 22, color: t.txt }} />
        <Text style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 17, color: t.mut }}>{QUESTION_LINE[k](q)}</Text>
      </PressableScale>
    </Animated.View>
  );
}

export function Awards({ s, byId, name, onOpen }: { s: Stats; byId: Map<number, Member>; name: Name; onOpen: (id: number) => void }) {
  useT();
  const groups = awardGroups(s, name);
  const questions = (['most_split', 'most_unanimous', 'hottest'] as const).filter((k) => s[k]);
  return (
    <>
      {groups.map((g) => {
        const qs = g.key === 'questions' ? questions : [];
        if (!g.items.length && !qs.length) return null;
        return (
          <Section key={g.key} title={g.title}>
            <View style={{ gap: 10 }}>
              {g.items.map((item, i) => (
                <AwardCard key={item.key} item={item} byId={byId} onOpen={onOpen} index={i} />
              ))}
              {qs.map((k, i) => (
                <QuestionCard key={k} k={k} q={s[k]!} index={g.items.length + i} />
              ))}
            </View>
          </Section>
        );
      })}
    </>
  );
}
