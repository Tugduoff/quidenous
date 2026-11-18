// Le Tripot (docs/TRIPOT.md): bets on who wins, with chips worth nothing. The "Parier" row and its sheet on the ballot and
// the waiting screen, the pot and its winners on the result (with a short shower of coins), the board on the stats page.
import { ChevronRight, Coins, Trophy } from 'lucide-react-native';
import { useMemo, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated, { useReducedMotion } from 'react-native-reanimated';
import Svg, { Circle } from 'react-native-svg';
import { dateFormat, useT } from '@/lib/i18n';
import { motion, usePop } from '@/lib/motion';
import { useHideVote } from '@/lib/device/prefs';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { CHIPS, type Tripot, useBet, useTripotBoard } from '@/lib/api/tripot';
import { burst, makeRand, Particles } from '@/components/celebration/engine';
import { useCelebrationClock } from '@/lib/hooks/use-celebration';
import { MemberGrid, MemberTile } from '@/components/vote/member-grid';
import { int, Section } from '@/components/stats/common';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { Dialog, DialogActions, DialogTitle } from '@/components/ui/dialog';
import type { Member } from '@/lib/api/group';
import type { Round } from '@/lib/api/rounds';

/** Gold that reads on both themes (the accent is for my own things). */
const GOLD = '#F2B705';
/** The coins of the shower: a few golds, and the darker ring stamped on them. */
const GOLD_COIN = { faces: ['#FFD54A', '#FFC107', '#F2B705', '#FFE082'], edge: '#B8860B' } as const;

/** On the ballot and the waiting screen: "Parier", then my bet and the pot. Nothing where the round has no Tripot. */
export function BetRow({ round, members, byId }: { round: Round; members: Member[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const [open, setOpen] = useState(false);
  // "Hide my vote" (friends looking at my screen) hides who I bet on too.
  const hidden = useHideVote();
  const tp = round.tripot;
  if (!tp || round.closed_at) return null;
  const mine = tp.my_bet;
  const target = mine ? byId.get(mine.target_id) : undefined;
  const title = mine
    ? hidden
      ? tr('tripot.myBetHidden', { amount: int(mine.amount) })
      : tr('tripot.myBet', { amount: int(mine.amount), name: target?.name ?? '?' })
    : tp.can_bet
      ? tr('tripot.bet')
      : tr('tripot.broke');
  return (
    <>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        accessibilityLabel={mine ? title : tr('tripot.betA11y')}
        disabled={!tp.can_bet}
        onPress={() => setOpen(true)}
        style={[
          {
            flexDirection: 'row',
            alignItems: 'center',
            gap: 12,
            paddingVertical: 10,
            paddingHorizontal: 12,
            borderRadius: 18,
            backgroundColor: t.card,
          },
          t.shadow,
        ]}>
        <View
          style={{
            width: 34,
            height: 34,
            borderRadius: 17,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: mine ? t.accSoft : t.card2,
          }}>
          <Coins size={18} color={mine ? t.accInk : GOLD} />
        </View>
        <View style={{ flex: 1, gap: 1 }}>
          <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: tp.can_bet || mine ? t.txt : t.mut }}>
            {title}
          </Text>
          {tp.bettors > 0 && (
            <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
              {tr('tripot.pot', { pot: int(tp.pot) })} · {tr('tripot.bettors', { count: tp.bettors })}
            </Text>
          )}
        </View>
        {tp.can_bet && <ChevronRight size={18} color={t.mut} />}
      </PressableScale>
      {tp.can_bet && <BetSheet visible={open} onClose={() => setOpen(false)} round={round} tripot={tp} members={members} />}
    </>
  );
}

/** Who (for a ranking: who ends up first), how much, confirm. One bet, no take-backs. */
function BetSheet({
  visible,
  onClose,
  round,
  tripot,
  members,
}: {
  visible: boolean;
  onClose: () => void;
  round: Round;
  tripot: Tripot;
  members: Member[];
}) {
  const t = useTheme();
  const tr = useT();
  const bet = useBet(round);
  const [target, setTarget] = useState<number | null>(null);
  const [amount, setAmount] = useState(() => Math.min(CHIPS[0], tripot.balance));
  const stakes = [...CHIPS.filter((c) => c < tripot.balance), tripot.balance];
  const chip = (value: number, label: string) => {
    const on = value === amount;
    return (
      <Pressable
        key={label}
        accessibilityRole="radio"
        accessibilityState={{ selected: on }}
        accessibilityLabel={tr('tripot.chipA11y', { amount: value })}
        onPress={() => setAmount(value)}
        style={{
          flex: 1,
          height: 40,
          borderRadius: t.radius.pill,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: on ? t.acc : t.card2,
        }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: on ? t.accTxt : t.txt }}>{label}</Text>
      </Pressable>
    );
  };
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title={tr(round.kind === 'rank' ? 'tripot.whoRank' : 'tripot.whoPick')} />
      <MemberGrid>
        {members.map((m) => (
          <MemberTile key={m.id} member={m} cols={4} plain role="radio" selected={m.id === target} onPress={() => setTarget(m.id)} />
        ))}
      </MemberGrid>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginTop: 4 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('tripot.amount')}</Text>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
          <Coins size={14} color={GOLD} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('tripot.balance', { count: tripot.balance })}</Text>
        </View>
      </View>
      <View style={{ flexDirection: 'row', gap: 8 }}>{stakes.map((v, i) => chip(v, i === stakes.length - 1 ? tr('tripot.allIn') : String(v)))}</View>
      <Text style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
        {tr('tripot.noValue')} {tr('tripot.final')}
      </Text>
      <DialogActions
        tone="acc"
        confirmLabel={tr('tripot.confirm', { amount: int(amount) })}
        disabled={target == null || amount < 1}
        busy={bet.isPending}
        onCancel={onClose}
        onConfirm={() => target != null && bet.mutate({ target, amount }, { onSuccess: onClose })}
      />
    </Dialog>
  );
}

/** A spinning gold coin for the shower. */
function Coin({ size, color }: { size: number; color: string }) {
  const r = size / 2;
  return (
    <Svg width={size} height={size}>
      <Circle cx={r} cy={r} r={r - 0.5} fill={color} />
      <Circle cx={r} cy={r} r={r * 0.58} fill="none" stroke={GOLD_COIN.edge} strokeWidth={1.2} />
    </Svg>
  );
}

/** Coins thrown up from the pot and falling back, about a second. Nothing under reduced motion. */
function CoinShower({ seed, width }: { seed: number; width: number }) {
  const clock = useCelebrationClock(1300);
  const list = useMemo(() => {
    const r = makeRand(seed * 31 + 7);
    return burst(r, {
      x: width - 56,
      y: 26,
      at: 0,
      count: 16,
      speed: [260, 560],
      dir: -100,
      spread: 110,
      g: 1300,
      k: 1.2,
      life: [900, 1250],
      fadeOut: 250,
      look: (rr) => {
        const size = rr.range(12, 18);
        return { shape: 'node', w: size, h: size, flip: rr.range(7, 15), node: <Coin size={size} color={rr.pick(GOLD_COIN.faces)} /> };
      },
    });
  }, [seed, width]);
  return <Particles clock={clock} list={list} />;
}

/** On the result: the pot and who won how much (or the bets coming back), with my own outcome. */
export function TripotResult({ round, byId }: { round: Round; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const reduced = useReducedMotion();
  const { me } = useSession();
  const [width, setWidth] = useState(0);
  const tp = round.tripot;
  if (!tp || !round.closed_at || tp.pot === 0) return null;
  const winners = tp.winners ?? [];
  const mine = tp.my_bet;
  const mineTarget = mine ? byId.get(mine.target_id) : undefined;
  const iWon = !!mine && !tp.refunded && (mine.payout ?? 0) > 0;
  return (
    <View onLayout={(e) => setWidth(e.nativeEvent.layout.width)} style={{ zIndex: 2 }}>
      <Card style={{ padding: 16, gap: 12 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <Coins size={16} color={GOLD} />
          <Text style={{ flex: 1, fontFamily: t.font.bold, fontSize: 12, letterSpacing: 0.6, color: t.mut, textTransform: 'uppercase' }}>
            {tr('tripot.name')}
          </Text>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('tripot.potOf', { count: tp.pot })}</Text>
        </View>
        {tp.refunded ? (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('tripot.refunded')}</Text>
        ) : (
          winners.map((w, i) => {
            const m = byId.get(w.member_id);
            return (
              <Animated.View
                key={w.member_id}
                entering={motion.stagger(i + 2)}
                accessible
                accessibilityLabel={tr('tripot.wonA11y', { name: m?.name ?? '?', won: w.won })}
                style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
                <Avatar member={m} size={32} />
                <Text
                  numberOfLines={1}
                  style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: w.member_id === me?.id ? t.accInk : t.txt }}>
                  {m?.name ?? '?'}
                </Text>
                <Won value={w.won} />
              </Animated.View>
            );
          })
        )}
        {mine && !tp.refunded && (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: iWon ? t.accInk : t.mut }}>
            {iWon
              ? tr('tripot.mineWon', { won: int(mine.payout ?? 0) })
              : tr('tripot.mineLost', { amount: int(mine.amount), name: mineTarget?.name ?? '?' })}
          </Text>
        )}
      </Card>
      {!reduced && !tp.refunded && winners.length > 0 && width > 0 && (
        <View pointerEvents="none" style={{ position: 'absolute', left: 0, top: 0, right: 0, bottom: 0 }}>
          <CoinShower seed={round.id} width={width} />
        </View>
      )}
    </View>
  );
}

/** "+44" in gold, with a small bump once it lands. */
function Won({ value }: { value: number }) {
  const t = useTheme();
  const tr = useT();
  const [landed, setLanded] = useState(false);
  const pop = usePop(landed, 1.25, landed);
  return (
    <Animated.View style={pop} onLayout={() => setTimeout(() => setLanded(true), 420)}>
      <Text style={{ fontFamily: t.font.display, fontSize: 20, color: GOLD, fontVariant: ['tabular-nums'] }}>
        {tr('tripot.won', { won: int(value) })}
      </Text>
    </Animated.View>
  );
}

/** Stats: this month's chips, most first, with last month's podium on top. */
export function TripotBoard({ byId, onOpen }: { byId: Map<number, Member>; onOpen: (id: number) => void }) {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  const { data } = useTripotBoard();
  if (!data || data.balances.length < 2) return null;
  const month = dateFormat({ month: 'long' }).format(new Date(`${data.last_month}-15T12:00:00Z`));
  return (
    <Section title={tr('tripot.board')}>
      <View style={{ gap: 10 }}>
        {data.podium.length > 0 && (
          <Card style={{ padding: 14, gap: 12 }}>
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
              <Trophy size={14} color={GOLD} />
              <Text style={{ fontFamily: t.font.bold, fontSize: 12, letterSpacing: 0.6, color: t.mut, textTransform: 'uppercase' }}>
                {tr('tripot.podium', { month: month.charAt(0).toUpperCase() + month.slice(1) })}
              </Text>
            </View>
            <View style={{ flexDirection: 'row', gap: 8 }}>
              {data.podium.map((p, i) => {
                const m = byId.get(p.member_id);
                return (
                  <PressableScale
                    key={p.member_id}
                    accessibilityRole="button"
                    accessibilityLabel={`${i + 1}, ${m?.name ?? '?'}, ${int(p.balance)} ${tr('tripot.chips', { count: p.balance })}`}
                    onPress={() => onOpen(p.member_id)}
                    style={{ flex: 1, alignItems: 'center', gap: 6 }}>
                    <View>
                      <Avatar member={m} size={i === 0 ? 48 : 40} />
                      <View
                        style={{
                          position: 'absolute',
                          right: -4,
                          bottom: -4,
                          width: 20,
                          height: 20,
                          borderRadius: 10,
                          alignItems: 'center',
                          justifyContent: 'center',
                          backgroundColor: i === 0 ? GOLD : t.card2,
                          borderWidth: 2,
                          borderColor: t.card,
                        }}>
                        <Text style={{ fontFamily: t.font.bold, fontSize: 10, color: i === 0 ? '#121216' : t.txt }}>{i + 1}</Text>
                      </View>
                    </View>
                    <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.txt, maxWidth: '100%' }}>
                      {m?.name ?? '?'}
                    </Text>
                    <Text style={{ fontFamily: t.font.display, fontSize: 16, color: t.txt }}>{int(p.balance)}</Text>
                  </PressableScale>
                );
              })}
            </View>
          </Card>
        )}
        <Card style={{ paddingVertical: 6, paddingHorizontal: 14 }}>
          {data.balances.map((b, i) => {
            const m = byId.get(b.member_id);
            const mine = b.member_id === me?.id;
            return (
              <Animated.View key={b.member_id} entering={motion.stagger(i)}>
                <PressableScale
                  scale={0.98}
                  accessibilityRole="button"
                  accessibilityLabel={`${i + 1}, ${m?.name ?? '?'}, ${int(b.balance)} ${tr('tripot.chips', { count: b.balance })}`}
                  onPress={() => onOpen(b.member_id)}
                  style={{
                    flexDirection: 'row',
                    alignItems: 'center',
                    gap: 10,
                    paddingVertical: 8,
                    borderTopWidth: i ? 1 : 0,
                    borderTopColor: t.line,
                  }}>
                  <Text style={{ width: 18, fontFamily: t.font.bold, fontSize: 13, color: t.mut, textAlign: 'center' }}>{i + 1}</Text>
                  <Avatar member={m} size={30} />
                  <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: mine ? t.accInk : t.txt }}>
                    {m?.name ?? '?'}
                  </Text>
                  <Text style={{ fontFamily: t.font.display, fontSize: 17, color: t.txt, fontVariant: ['tabular-nums'] }}>{int(b.balance)}</Text>
                  <Coins size={14} color={GOLD} />
                </PressableScale>
              </Animated.View>
            );
          })}
        </Card>
        <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut, textAlign: 'center' }}>
          {tr('tripot.reset', { start: data.start })}
        </Text>
      </View>
    </Section>
  );
}
