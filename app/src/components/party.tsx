import { Check, Crown } from 'lucide-react-native';
import { type ReactNode, useEffect } from 'react';
import { Text, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';
import Animated, {
  Easing,
  FadeInDown,
  FadeInUp,
  LayoutAnimationConfig,
  ReduceMotion,
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withDelay,
  withTiming,
  ZoomIn,
} from 'react-native-reanimated';

import { Avatar, AvatarStack, PressableScale } from '@/components/ui';
import type { Member } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { EASE, motion, useArrivals } from '@/lib/motion';
import { type PartyPlayer, type PartyRound, partyJoinLink, REACTIONS, tallies } from '@/lib/party';
import { useTheme } from '@/lib/theme';

/** The join QR code on its white tile. */
export function JoinQr({ code, size }: { code: string; size: number }) {
  return (
    <View style={{ alignSelf: 'center', padding: Math.round(size * 0.06), borderRadius: 16, backgroundColor: '#fff' }}>
      <QRCode value={partyJoinLink(code)} size={size} color="#121216" backgroundColor="#fff" />
    </View>
  );
}

/** Seconds left and a bar running down; red for the last five. `big`: the big screen. */
export function Countdown({ ms, total, big }: { ms: number | null; total: number; big?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const reduced = useReducedMotion();
  const frac = ms == null ? 1 : Math.min(1, ms / Math.max(1, total * 1000));
  const w = useSharedValue(frac);
  useEffect(() => {
    // The bar is the clock itself, so it moves under reduced motion too, only in steps.
    w.value = reduced ? frac : withTiming(frac, { duration: 200, easing: Easing.linear, reduceMotion: ReduceMotion.Never });
  }, [frac, reduced, w]);
  const bar = useAnimatedStyle(() => ({ transform: [{ scaleX: w.value }] }));
  const s = ms == null ? null : Math.ceil(ms / 1000);
  const urgent = s != null && s <= 5;
  const color = urgent ? t.tag : t.accInk;
  const h = big ? 16 : 8;
  return (
    <View
      accessibilityRole="timer"
      accessibilityLabel={s != null ? tr('party.vote.secondsLeft', { count: s }) : undefined}
      style={{ flexDirection: 'row', alignItems: 'center', gap: big ? 24 : 12 }}>
      <View style={{ flex: 1, height: h, borderRadius: h, backgroundColor: t.card2, overflow: 'hidden' }}>
        <Animated.View style={[{ width: '100%', height: '100%', borderRadius: h, backgroundColor: color, transformOrigin: 'left' }, bar]} />
      </View>
      <Text
        style={{
          minWidth: big ? 120 : 44,
          textAlign: 'right',
          fontFamily: t.font.display,
          fontSize: big ? 88 : 30,
          lineHeight: big ? 96 : 34,
          color: urgent ? t.tag : t.txt,
          fontVariant: ['tabular-nums'],
        }}>
        {s ?? ''}
      </Text>
    </View>
  );
}

/** Faces of who voted and "3/5 ont voté". */
export function VotedLine({ round, byId, big }: { round: PartyRound; byId: Map<number, Member>; big?: boolean }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: big ? 16 : 8 }}>
      {round.voted.length > 0 && <AvatarStack members={round.voted.map((id) => byId.get(id))} size={big ? 48 : 28} max={big ? 10 : 6} />}
      <Text style={{ fontFamily: big ? t.font.display : t.font.semibold, fontSize: big ? 36 : 15, color: t.mut }}>
        {tr('party.vote.voted', { count: round.voted.filter((id) => round.eligible.includes(id)).length, total: round.eligible.length })}
      </Text>
    </View>
  );
}

const sys = ReduceMotion.System;
// Fresh builders: a preset's own `.delay()` would change it for the whole app.
const crownIn = FadeInUp.duration(260).easing(EASE).delay(500).reduceMotion(sys);
const nameIn = FadeInDown.duration(280).easing(EASE).delay(650).reduceMotion(sys);
const faceIn = (i: number) =>
  ZoomIn.springify()
    .damping(11)
    .stiffness(170)
    .delay(150 + i * 140)
    .reduceMotion(sys);

/** The winner (or the tied ones), big: faces spring in with a ring bursting out, the crown drops, then the name. */
export function WinnerReveal({ round, byId, size = 120 }: { round: PartyRound; byId: Map<number, Member>; size?: number }) {
  const t = useTheme();
  const tr = useT();
  const list = tallies(round);
  const top = list[0]?.count ?? 0;
  const winners = top > 0 ? list.filter((r) => r.count === top).map((r) => byId.get(r.target_id) ?? fallback(r.target_id)) : [];
  if (!winners.length)
    return (
      <Text style={{ textAlign: 'center', fontFamily: t.font.display, fontSize: size * 0.24, color: t.mut, paddingVertical: size * 0.2 }}>
        {tr('party.reveal.nobody')}
      </Text>
    );
  const tie = winners.length > 1;
  const face = tie ? Math.round(size * (winners.length > 2 ? 0.55 : 0.75)) : size;
  return (
    <View accessibilityRole="header" style={{ alignItems: 'center', gap: size * 0.1 }}>
      <Animated.View entering={crownIn}>
        <Crown size={Math.round(size * 0.34)} color={winners[0].color} fill={winners[0].color} />
      </Animated.View>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', justifyContent: 'center', gap: size * 0.14 }}>
        {winners.map((m, i) => (
          <Animated.View key={m.id} entering={faceIn(i)}>
            <Burst color={m.color} size={face} delay={300 + i * 140}>
              <Avatar member={m} size={face} />
            </Burst>
          </Animated.View>
        ))}
      </View>
      <Animated.View entering={nameIn} style={{ alignItems: 'center', gap: 2 }}>
        <Text
          numberOfLines={2}
          style={{ textAlign: 'center', fontFamily: t.font.display, fontSize: size * 0.3, lineHeight: size * 0.34, letterSpacing: -1, color: t.txt }}>
          {winners.map((m) => m.name).join(' · ')}
        </Text>
        {tie && <Text style={{ fontFamily: t.font.semibold, fontSize: Math.max(14, size * 0.13), color: t.mut }}>{tr('party.reveal.tie')}</Text>}
      </Animated.View>
    </View>
  );
}

const fallback = (id: number): Member => ({ id, name: '?', color: '#8D8D99', photo_url: null });

/** A ring in the person's colour growing out from behind the face and fading, once. Nothing under reduced motion. */
function Burst({ color, size, delay, children }: { color: string; size: number; delay: number; children: ReactNode }) {
  const reduced = useReducedMotion();
  const p = useSharedValue(0);
  useEffect(() => {
    if (!reduced) p.value = withDelay(delay, withTiming(1, { duration: 900, easing: EASE }));
  }, [delay, reduced, p]);
  const ring = useAnimatedStyle(() => ({ opacity: p.value === 0 ? 0 : 0.6 * (1 - p.value), transform: [{ scale: 1 + p.value * 0.7 }] }));
  return (
    <View style={{ width: size, height: size }}>
      <Animated.View
        pointerEvents="none"
        style={[
          { position: 'absolute', width: size, height: size, borderRadius: size / 2, borderWidth: Math.max(3, size * 0.05), borderColor: color },
          ring,
        ]}
      />
      {children}
    </View>
  );
}

/** The six quick reactions under a result, with their counts; mine are highlighted. */
export function Reactions({ round, onReact, disabled }: { round: PartyRound; onReact?: (emoji: string) => void; disabled?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const counts = new Map((round.reactions ?? []).map((r) => [r.emoji, r]));
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'center', gap: 6 }}>
      {REACTIONS.map((emoji) => {
        const r = counts.get(emoji);
        const mine = !!r?.mine;
        return (
          <PressableScale
            key={emoji}
            scale={0.9}
            accessibilityRole="button"
            accessibilityLabel={tr('party.reveal.react', { emoji })}
            accessibilityState={{ selected: mine }}
            disabled={disabled || !onReact}
            onPress={() => onReact?.(emoji)}
            style={{
              flex: 1,
              maxWidth: 72,
              height: 44,
              paddingHorizontal: 4,
              borderRadius: 22,
              flexDirection: 'row',
              alignItems: 'center',
              justifyContent: 'center',
              gap: 4,
              borderWidth: 2,
              borderColor: mine ? t.accInk : 'transparent',
              backgroundColor: mine ? t.accSoft : t.card,
            }}>
            <Text style={{ fontSize: 19 }}>{emoji}</Text>
            {!!r?.count && <Text style={{ fontFamily: t.font.bold, fontSize: 14, color: mine ? t.accInk : t.mut }}>{r.count}</Text>}
          </PressableScale>
        );
      })}
    </View>
  );
}

/** "Suivante", filling up as people tap it toward the count that opens the next question. The owner's tap opens it right away. */
export function NextButton({ round, owner, busy, onPress }: { round: PartyRound; owner: boolean; busy?: boolean; onPress: () => void }) {
  const t = useTheme();
  const tr = useT();
  const taps = round.next?.taps.length ?? 0;
  const needed = Math.max(1, round.next?.needed ?? 1);
  const mine = !!round.next?.mine;
  const fill = useSharedValue(Math.min(1, taps / needed));
  useEffect(() => {
    fill.value = withTiming(Math.min(1, taps / needed), { duration: 300, easing: EASE, reduceMotion: sys });
  }, [taps, needed, fill]);
  const bar = useAnimatedStyle(() => ({ transform: [{ scaleX: fill.value }] }));
  const solid = owner;
  const ink = solid ? t.accTxt : t.txt;
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={tr('party.reveal.nextA11y', { count: taps, needed })}
      accessibilityState={{ selected: mine, busy: !!busy }}
      disabled={busy}
      onPress={onPress}
      style={{
        height: 58,
        borderRadius: 999,
        overflow: 'hidden',
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 10,
        backgroundColor: solid ? t.acc : t.card2,
        borderWidth: 2,
        borderColor: mine ? t.accInk : 'transparent',
      }}>
      {!solid && (
        <Animated.View
          style={[
            { position: 'absolute', left: 0, top: 0, bottom: 0, width: '100%', backgroundColor: t.acc, opacity: 0.45, transformOrigin: 'left' },
            bar,
          ]}
        />
      )}
      {mine && <Check size={20} color={ink} />}
      <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: ink }}>{tr('party.reveal.next')}</Text>
      <Text style={{ fontFamily: t.font.display, fontSize: 16, color: ink, opacity: 0.7, fontVariant: ['tabular-nums'] }}>
        {taps}/{needed}
      </Text>
    </PressableScale>
  );
}

/** The players as chips (face and name); the owner wears a crown, and someone arriving pops in. */
export function PlayerChips({ players, big }: { players: PartyPlayer[]; big?: boolean }) {
  const t = useTheme();
  const isNew = useArrivals(players.map((p) => p.id));
  const size = big ? 52 : 30;
  return (
    <LayoutAnimationConfig skipEntering>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: big ? 14 : 8, justifyContent: big ? 'center' : 'flex-start' }}>
        {players.map((p) => (
          <Animated.View
            key={p.id}
            entering={isNew(p.id) ? motion.pop : undefined}
            layout={motion.layout}
            style={[
              {
                flexDirection: 'row',
                alignItems: 'center',
                gap: big ? 12 : 8,
                paddingLeft: 4,
                paddingRight: big ? 22 : 14,
                paddingVertical: 4,
                borderRadius: 999,
                backgroundColor: t.card,
                maxWidth: '100%',
              },
              t.shadow,
            ]}>
            <Avatar member={p} size={size} />
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: big ? 24 : 15, color: t.txt }}>
              {p.name}
            </Text>
            {p.role === 'owner' && <Crown size={big ? 20 : 14} color={t.accInk} />}
          </Animated.View>
        ))}
      </View>
    </LayoutAnimationConfig>
  );
}
