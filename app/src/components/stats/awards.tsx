import { router } from 'expo-router';
import { ChevronRight } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { Avatar, AvatarStack, PressableScale, QuestionText } from '@/components/ui';
import type { Award, Duo, Member, Pair, QuestionRef, RankAward, Stats } from '@/lib/api';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

import { AWARD_META, Caps, dec, duration, iconFor, int, pct, plural, Section } from './common';

/** One award ready to show: who, the figure, and a quip. */
type Item = { key: string; ids: number[]; who: string; big: string; unit: string; quip: string };
type Name = (id: number) => string;

const n = (v: number, one: string, many: string) => ({ big: int(v), unit: plural(v, one, many) });

/** Single-person awards: the figure and the quip, from its value (and total, for ratios). */
const SOLO: Record<string, (a: Award) => { big: string; unit: string; quip: string }> = {
  most_chosen: (a) => ({ ...n(a.value, 'vote', 'votes'), quip: 'Autographes sur demande.' }),
  most_crowned: (a) => ({ ...n(a.value, 'victoire', 'victoires'), quip: 'La couronne commence à peser.' }),
  streak: (a) => ({ big: int(a.value), unit: "d'affilée", quip: "Quelqu'un pour arrêter ça ?" }),
  longest_streak: (a) => ({ big: int(a.value), unit: 'de suite', quip: 'Record à battre.' }),
  comeback: (a) => ({ big: int(a.value), unit: 'de disette', quip: "On n'y croyait plus." }),
  narcissist: (a) => ({ big: int(a.value), unit: 'pour soi', quip: "On n'est jamais mieux servi." }),
  contrarian: (a) => ({ big: pct(a.value, a.total), unit: 'contre la foule', quip: 'La majorité ? Très peu pour moi.' }),
  sheep: (a) => ({ big: pct(a.value, a.total), unit: 'avec la foule', quip: 'Bêêê.' }),
  kingmaker: (a) => ({ big: int(a.value), unit: 'à une voix', quip: "Le vrai pouvoir est dans l'ombre." }),
  fastest: (a) => ({ big: duration(a.value), unit: 'par vote', quip: 'Lire la question ? Optionnel.' }),
  slowest: (a) => ({ big: duration(a.value), unit: 'par vote', quip: 'Réflexion intense ou sieste ?' }),
  last_voter: (a) => ({ big: int(a.value), unit: 'fois dernier', quip: "Le suspense, c'est un art." }),
  night_owl: (a) => ({ ...n(a.value, 'vote de nuit', 'votes de nuit'), quip: 'Va dormir.' }),
  early_bird: (a) => ({ big: int(a.value), unit: 'avant 9 h', quip: "Le café, c'est pour après." }),
  chatterbox: (a) => ({ ...n(a.value, 'message', 'messages'), quip: 'Le clavier fume encore.' }),
  silent: (a) => ({ ...n(a.value, 'message', 'messages'), quip: 'Observe tout, ne dit rien.' }),
  nudger: (a) => ({ ...n(a.value, 'relance', 'relances'), quip: 'Ding. Ding. Ding.' }),
  most_nudged: (a) => ({ ...n(a.value, 'relance', 'relances'), quip: 'Le téléphone vibre encore.' }),
  author: (a) => ({ ...n(a.value, 'question', 'questions'), quip: 'Merci pour le contenu.' }),
};

const rank = (a: RankAward, quip: string) => ({ big: `${dec(a.avg)}e`, unit: 'en moyenne', quip });

/** The award cards, in titled groups; an award nobody holds this period is left out, and so is an empty group. */
export function awardGroups(s: Stats, name: Name) {
  const solo = (key: string): Item | null => {
    const a = s[key as keyof Stats] as Award | null;
    return a ? { key, ids: [a.member_id], who: name(a.member_id), ...SOLO[key](a) } : null;
  };
  const duo = (key: string, d: Duo | null, unit: (d: Duo) => [string, string], quip: string): Item | null =>
    d ? { key, ids: [d.voter_id, d.target_id], who: `${name(d.voter_id)} pour ${name(d.target_id)}`, big: unit(d)[0], unit: unit(d)[1], quip } : null;
  const pair = (p: Pair | null): Item | null =>
    p
      ? {
          key: 'twins',
          ids: [p.a_id, p.b_id],
          who: `${name(p.a_id)} et ${name(p.b_id)}`,
          big: `${p.value}/${p.total}`,
          unit: 'votes pareils',
          quip: 'Télépathie confirmée.',
        }
      : null;
  const ranked = (key: 'best_rank' | 'worst_rank', quip: string): Item | null => {
    const a = s[key];
    return a ? { key, ids: [a.member_id], who: name(a.member_id), ...rank(a, quip) } : null;
  };
  const ghosts: Item | null = s.never_chosen.length
    ? { key: 'never_chosen', ids: s.never_chosen, who: s.never_chosen.map(name).join(', '), big: '0', unit: 'vote', quip: 'Votre heure viendra.' }
    : null;
  const groups: { title: string; items: (Item | null)[] }[] = [
    {
      title: 'Les titres',
      items: [
        solo('most_chosen'),
        solo('most_crowned'),
        solo('streak'),
        solo('longest_streak'),
        solo('comeback'),
        ranked('best_rank', 'Abonnement podium.'),
        ranked('worst_rank', 'Il faut bien fermer la marche.'),
        ghosts,
      ],
    },
    {
      title: 'Les votes',
      items: [
        duo('duo', s.duo, (d) => [int(d.value), plural(d.value, 'vote', 'votes')], 'On ne dit rien, on note.'),
        duo('loyal', s.loyal, (d) => [pct(d.value, d.total), 'des votes'], 'Fidélité absolue.'),
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
    { title: 'Le chat', items: [solo('chatterbox'), solo('silent'), solo('nudger'), solo('most_nudged')] },
    { title: 'Les questions', items: [solo('author')] },
  ];
  return groups.map((g) => ({ title: g.title, items: g.items.filter((i): i is Item => !!i) }));
}

function AwardCard({ item, byId, onOpen, index }: { item: Item; byId: Map<number, Member>; onOpen: (id: number) => void; index: number }) {
  const t = useTheme();
  const meta = AWARD_META[item.key];
  const Icon = iconFor(meta?.icon);
  const people = item.ids.map((id) => byId.get(id));
  const tint = people.length === 1 ? (people[0]?.color ?? t.accInk) : t.accInk;
  return (
    <Animated.View entering={motion.stagger(index)}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        accessibilityLabel={`${meta?.title}: ${item.who}, ${item.big} ${item.unit}`}
        onPress={() => onOpen(item.ids[0])}
        style={[
          { flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14, borderRadius: t.radius.card, backgroundColor: t.card },
          t.shadow,
        ]}>
        {people.length === 1 ? <Avatar member={people[0]} size={44} /> : <AvatarStack members={people} size={34} ring={t.card} max={3} />}
        <View style={{ flex: 1, gap: 3 }}>
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
            <Icon size={13} color={tint} strokeWidth={2.5} />
            <Caps size={11}>{meta?.title}</Caps>
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
  most_split: (q) => `${q.value} personnes citées sur ${q.total} votes. Débat ouvert.`,
  most_unanimous: (q) => `${q.total} votes, zéro débat.`,
  hottest: (q) => `${q.value} ${plural(q.value, 'message', 'messages')}. Ça a chauffé.`,
};

/** A question award: its title, the question, one line; opens the question. */
function QuestionCard({ k, q, index }: { k: string; q: QuestionRef; index: number }) {
  const t = useTheme();
  const meta = AWARD_META[k];
  const Icon = iconFor(meta?.icon);
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
            <Caps size={11}>{meta?.title}</Caps>
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
  const groups = awardGroups(s, name);
  const questions = (['most_split', 'most_unanimous', 'hottest'] as const).filter((k) => s[k]);
  return (
    <>
      {groups.map((g) => {
        const qs = g.title === 'Les questions' ? questions : [];
        if (!g.items.length && !qs.length) return null;
        return (
          <Section key={g.title} title={g.title}>
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
