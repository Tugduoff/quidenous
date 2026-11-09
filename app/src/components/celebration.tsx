import { useEffect, useMemo, useRef } from 'react';
import { Animated, Easing, Pressable, StyleSheet, Text, useWindowDimensions, View } from 'react-native';

import { milestone as describe, type Milestone } from '@/lib/milestones';
import { useTheme } from '@/lib/theme';

const CONFETTI = ['#C8FF3D', '#FF4FA3', '#5AC8FA', '#FFB547', '#B48CFF', '#5EF2B0', '#FF7A45'];
const GOLD = ['#FFD54A', '#FFB300', '#FFF1B8', '#E8A317', '#FFE082'];
const TRICOLORE = ['#0055A4', '#FFFFFF', '#EF4135'];
const PIXEL = ['#FFE600', '#FF2A6D', '#05D9E8', '#3DFF6E', '#B48CFF', '#FF7A45'];
const MATRIX_CHARS = '0123456789ABCDEF01010110';

const rand = (a: number, b: number) => a + Math.random() * (b - a);
const pick = <T,>(xs: T[]) => xs[Math.floor(Math.random() * xs.length)];

/** Per theme: backdrop, number color, how long it lasts. */
function look(m: Milestone, t: ReturnType<typeof useTheme>) {
  switch (m.theme) {
    case 'gold':
      return { bg: t.bg, number: '#E8A317', duration: m.epic ? 6500 : 4800 };
    case 'devil':
      return { bg: '#1A0000', number: '#FF2A2A', text: '#FFD6D6', duration: 6000 };
    case 'jackpot':
      return { bg: '#120A2A', number: '#FFD54A', text: '#FFFFFF', duration: 6000 };
    case 'glitch':
      return { bg: '#050508', number: '#F5F5F7', text: '#F5F5F7', duration: 4800 };
    case 'matrix':
      return { bg: '#000000', number: '#3DFF6E', text: '#B8FFC9', duration: 6000 };
    case 'rocket':
      return { bg: '#0B1026', number: '#FFFFFF', text: '#E6ECFF', duration: 5200 };
    case 'hearts':
      return { bg: t.bg, number: '#FF4FA3', duration: 5000 };
    case 'pixel':
      return { bg: '#0F0F1B', number: '#FFE600', text: '#FFFFFF', duration: m.epic ? 6000 : 5000 };
    case 'siren':
      return { bg: '#0A0A12', number: '#FFFFFF', text: '#FFFFFF', duration: 5000 };
    case 'tricolore':
      return { bg: '#0B1026', number: '#FFFFFF', text: '#E6ECFF', duration: 6500 };
    case 'night':
      return { bg: '#0B1026', number: '#FFE9A8', text: '#E6ECFF', duration: 6000 };
    default:
      return { bg: t.bg, number: t.accInk, duration: 4200 };
  }
}

/** Shared clock: 0 → 1 over the whole party. Pieces read it with their own delay. */
function useClock(duration: number) {
  const v = useRef(new Animated.Value(0)).current;
  useEffect(() => {
    Animated.timing(v, { toValue: 1, duration, easing: Easing.linear, useNativeDriver: true }).start();
  }, [v, duration]);
  return v;
}

/** Things falling from the top: paper confetti, or emoji when `emoji` is given. */
function Rain({
  clock,
  colors,
  emoji,
  count,
  rise,
}: {
  clock: Animated.Value;
  colors: string[];
  emoji?: string[];
  count: number;
  rise?: boolean;
}) {
  const { width, height } = useWindowDimensions();
  const pieces = useMemo(
    () =>
      Array.from({ length: count }, (_, i) => ({
        x: rand(0, width),
        drift: rand(-80, 80),
        start: rand(0, 0.45),
        span: rand(0.45, 0.75),
        spin: (Math.random() < 0.5 ? -1 : 1) * rand(emoji ? 30 : 360, emoji ? 160 : 1080),
        w: rand(6, 12),
        h: rand(10, 20),
        round: i % 4 === 0,
        color: pick(colors),
        glyph: emoji ? pick(emoji) : null,
        size: rand(22, 36),
      })),
    [count, width, colors, emoji],
  );
  const [from, to] = rise ? [height + 40, -80] : [-40, height + 60];
  return (
    <>
      {pieces.map((p, i) => {
        const end = Math.min(1, p.start + p.span);
        const progress = clock.interpolate({ inputRange: [0, p.start, end, 1], outputRange: [0, 0, 1, 1] });
        const transform = [
          { translateY: progress.interpolate({ inputRange: [0, 1], outputRange: [from, to] }) },
          { translateX: progress.interpolate({ inputRange: [0, 1], outputRange: [0, p.drift] }) },
          { rotate: progress.interpolate({ inputRange: [0, 1], outputRange: ['0deg', `${p.spin}deg`] }) },
        ];
        const opacity = rise ? progress.interpolate({ inputRange: [0, 0.1, 0.8, 1], outputRange: [0, 1, 1, 0] }) : 1;
        return p.glyph ? (
          <Animated.Text key={i} style={{ position: 'absolute', left: p.x, top: 0, fontSize: p.size, opacity, transform }}>
            {p.glyph}
          </Animated.Text>
        ) : (
          <Animated.View
            key={i}
            style={{
              position: 'absolute',
              left: p.x,
              top: 0,
              width: p.w,
              height: p.round ? p.w : p.h,
              borderRadius: p.round ? p.w / 2 : 2,
              backgroundColor: p.color,
              opacity,
              transform,
            }}
          />
        );
      })}
    </>
  );
}

/** Fireworks: sparks flying out of a few points, one burst after the other. */
function Fireworks({ clock, colors, bursts }: { clock: Animated.Value; colors: string[]; bursts: number }) {
  const { width, height } = useWindowDimensions();
  const sparks = useMemo(
    () =>
      Array.from({ length: bursts }, (_, b) => {
        const cx = rand(width * 0.2, width * 0.8);
        const cy = rand(height * 0.12, height * 0.4);
        const start = (b / bursts) * 0.7;
        return Array.from({ length: 22 }, (_, i) => {
          const angle = (i / 22) * Math.PI * 2 + rand(-0.1, 0.1);
          const dist = rand(70, 140);
          return { cx, cy, start, dx: Math.cos(angle) * dist, dy: Math.sin(angle) * dist, color: pick(colors) };
        });
      }).flat(),
    [bursts, width, height, colors],
  );
  return (
    <>
      {sparks.map((s, i) => {
        const end = Math.min(1, s.start + 0.22);
        const p = clock.interpolate({ inputRange: [0, s.start, end, 1], outputRange: [0, 0, 1, 1] });
        return (
          <Animated.View
            key={i}
            style={{
              position: 'absolute',
              left: s.cx,
              top: s.cy,
              width: 6,
              height: 6,
              borderRadius: 3,
              backgroundColor: s.color,
              opacity: p.interpolate({ inputRange: [0, 0.01, 0.7, 1], outputRange: [0, 1, 1, 0] }),
              transform: [
                { translateX: p.interpolate({ inputRange: [0, 1], outputRange: [0, s.dx] }) },
                // A little gravity: sparks drop at the end.
                { translateY: p.interpolate({ inputRange: [0, 0.6, 1], outputRange: [0, s.dy * 0.8, s.dy + 40] }) },
              ],
            }}
          />
        );
      })}
    </>
  );
}

/** Columns of green characters raining down. */
function MatrixRain({ clock }: { clock: Animated.Value }) {
  const { width, height } = useWindowDimensions();
  const cols = useMemo(
    () =>
      Array.from({ length: Math.ceil(width / 22) }, (_, i) => ({
        x: i * 22,
        start: rand(0, 0.4),
        text: Array.from({ length: 28 }, () => pick([...MATRIX_CHARS])).join('\n'),
      })),
    [width],
  );
  return (
    <>
      {cols.map((c, i) => {
        const p = clock.interpolate({ inputRange: [0, c.start, 1], outputRange: [0, 0, 1] });
        return (
          <Animated.Text
            key={i}
            style={{
              position: 'absolute',
              left: c.x,
              top: 0,
              color: '#3DFF6E',
              opacity: 0.55,
              fontSize: 16,
              lineHeight: 20,
              fontFamily: 'monospace',
              transform: [{ translateY: p.interpolate({ inputRange: [0, 1], outputRange: [-600, height + 200] }) }],
            }}>
            {c.text}
          </Animated.Text>
        );
      })}
    </>
  );
}

/** Slow rotating sun rays behind the number. */
function Rays({ color }: { color: string }) {
  const spin = useRef(new Animated.Value(0)).current;
  useEffect(() => {
    Animated.loop(Animated.timing(spin, { toValue: 1, duration: 9000, easing: Easing.linear, useNativeDriver: true })).start();
  }, [spin]);
  const rotate = spin.interpolate({ inputRange: [0, 1], outputRange: ['0deg', '360deg'] });
  return (
    <Animated.View
      style={{ position: 'absolute', width: 440, height: 440, alignItems: 'center', justifyContent: 'center', transform: [{ rotate }] }}>
      {Array.from({ length: 12 }, (_, i) => (
        <View
          key={i}
          style={{
            position: 'absolute',
            width: 10,
            height: 440,
            backgroundColor: color,
            opacity: 0.1,
            borderRadius: 5,
            transform: [{ rotate: `${i * 15}deg` }],
          }}
        />
      ))}
    </Animated.View>
  );
}

/** The digits of a slot machine, each reel stopping a bit after the previous one. */
function Reels({ value, size, color }: { value: number; size: number; color: string }) {
  const digits = String(value).split('');
  const lineH = size * 1.04;
  return (
    <View style={{ flexDirection: 'row', gap: 6 }}>
      {digits.map((d, i) => (
        <Reel key={i} digit={Number(d)} delay={i * 450} lineH={lineH} size={size} color={color} />
      ))}
    </View>
  );
}

function Reel({ digit, delay, lineH, size, color }: { digit: number; delay: number; lineH: number; size: number; color: string }) {
  const roll = useRef(new Animated.Value(0)).current;
  const strip = useMemo(() => [...Array.from({ length: 18 }, (_, i) => (i * 7 + digit + 3) % 10), digit], [digit]);
  useEffect(() => {
    Animated.timing(roll, { toValue: 1, duration: 1500 + delay, easing: Easing.out(Easing.cubic), useNativeDriver: true }).start();
  }, [roll, delay]);
  return (
    <View style={{ height: lineH, overflow: 'hidden', borderRadius: 12, backgroundColor: '#ffffff14', paddingHorizontal: 6 }}>
      <Animated.View
        style={{ transform: [{ translateY: roll.interpolate({ inputRange: [0, 1], outputRange: [0, -(strip.length - 1) * lineH] }) }] }}>
        {strip.map((n, i) => (
          <Text key={i} style={{ height: lineH, fontFamily: 'BricolageGrotesque_800ExtraBold', fontSize: size, lineHeight: lineH, color }}>
            {n}
          </Text>
        ))}
      </Animated.View>
    </View>
  );
}

/** Square pixels dropping one cell at a time, like an old console. */
function PixelRain({ clock }: { clock: Animated.Value }) {
  const { width, height } = useWindowDimensions();
  const STEPS = 14;
  const pixels = useMemo(
    () =>
      Array.from({ length: 60 }, () => ({
        x: Math.round(rand(0, width) / 12) * 12,
        start: rand(0, 0.5),
        size: pick([10, 12, 16]),
        color: pick(PIXEL),
      })),
    [width],
  );
  return (
    <>
      {pixels.map((p, i) => {
        const end = Math.min(1, p.start + 0.45);
        // A staircase: hold, jump, hold, jump…
        const input: number[] = [0];
        const output: number[] = [-40];
        for (let k = 0; k <= STEPS; k++) {
          const at = p.start + ((end - p.start) * k) / STEPS;
          const y = -40 + ((height + 80) * k) / STEPS;
          input.push(at, at + 0.0001);
          output.push(output[output.length - 1], y);
        }
        input.push(1);
        output.push(height + 40);
        const ys = input.map((v, j) => ({ v: Math.min(1, v), y: output[j] }));
        const clean = ys.filter((e, j) => j === 0 || e.v > ys[j - 1].v);
        return (
          <Animated.View
            key={i}
            style={{
              position: 'absolute',
              left: p.x,
              top: 0,
              width: p.size,
              height: p.size,
              backgroundColor: p.color,
              transform: [{ translateY: clock.interpolate({ inputRange: clean.map((e) => e.v), outputRange: clean.map((e) => e.y) }) }],
            }}
          />
        );
      })}
    </>
  );
}

/** Twinkling stars on a night sky. */
function Stars() {
  const { width, height } = useWindowDimensions();
  const twinkle = useRef(new Animated.Value(0)).current;
  useEffect(() => {
    Animated.loop(Animated.timing(twinkle, { toValue: 1, duration: 1800, easing: Easing.linear, useNativeDriver: true })).start();
  }, [twinkle]);
  const stars = useMemo(
    () => Array.from({ length: 70 }, (_, i) => ({ x: rand(0, width), y: rand(0, height), size: rand(1.5, 4), group: i % 3 })),
    [width, height],
  );
  const phases = [
    twinkle.interpolate({ inputRange: [0, 0.5, 1], outputRange: [0.2, 1, 0.2] }),
    twinkle.interpolate({ inputRange: [0, 0.5, 1], outputRange: [1, 0.2, 1] }),
    twinkle.interpolate({ inputRange: [0, 0.25, 0.75, 1], outputRange: [0.6, 0.15, 1, 0.6] }),
  ];
  return (
    <>
      {stars.map((s, i) => (
        <Animated.View
          key={i}
          style={{
            position: 'absolute',
            left: s.x,
            top: s.y,
            width: s.size,
            height: s.size,
            borderRadius: s.size,
            backgroundColor: '#FFF6D8',
            opacity: phases[s.group],
          }}
        />
      ))}
    </>
  );
}

/** A rocket taking off from the bottom, leaving a trail of smoke. */
function Rocket({ clock }: { clock: Animated.Value }) {
  const { width, height } = useWindowDimensions();
  const [from, to] = [height + 80, -220];
  const launch = [0.12, 0.62];
  const y = clock.interpolate({ inputRange: [0, launch[0], launch[1], 1], outputRange: [from, from, to, to] });
  const puffs = useMemo(() => Array.from({ length: 16 }, (_, i) => ({ k: i / 16, dx: rand(-18, 18), size: rand(26, 46) })), []);
  return (
    <>
      {puffs.map((p, i) => {
        // Each puff appears when the rocket passes its height, then swells and fades.
        const at = launch[0] + (launch[1] - launch[0]) * p.k;
        const py = from + (to - from) * p.k + 70;
        const life = clock.interpolate({ inputRange: [0, at, Math.min(1, at + 0.3), 1], outputRange: [0, 0, 1, 1] });
        return (
          <Animated.View
            key={i}
            style={{
              position: 'absolute',
              left: width / 2 - p.size / 2 + p.dx,
              top: py,
              width: p.size,
              height: p.size,
              borderRadius: p.size,
              backgroundColor: '#C9CEDF',
              opacity: life.interpolate({ inputRange: [0, 0.01, 1], outputRange: [0, 0.55, 0] }),
              transform: [{ scale: life.interpolate({ inputRange: [0, 1], outputRange: [0.4, 2.2] }) }],
            }}
          />
        );
      })}
      <Animated.Text
        style={{ position: 'absolute', left: width / 2 - 40, top: 0, fontSize: 80, transform: [{ translateY: y }, { rotate: '-45deg' }] }}>
        🚀
      </Animated.Text>
    </>
  );
}

/** Police lights: the whole screen flashes red, then blue. */
function SirenLights() {
  const flash = useRef(new Animated.Value(0)).current;
  useEffect(() => {
    const anim = Animated.loop(
      Animated.sequence([
        Animated.timing(flash, { toValue: 1, duration: 260, useNativeDriver: true }),
        Animated.timing(flash, { toValue: 0, duration: 260, useNativeDriver: true }),
      ]),
    );
    anim.start();
    return () => anim.stop();
  }, [flash]);
  return (
    <>
      <Animated.View
        style={[
          StyleSheet.absoluteFill,
          { backgroundColor: '#FF1E3C', opacity: flash.interpolate({ inputRange: [0, 1], outputRange: [0.45, 0] }) },
        ]}
      />
      <Animated.View
        style={[
          StyleSheet.absoluteFill,
          { backgroundColor: '#1E5BFF', opacity: flash.interpolate({ inputRange: [0, 1], outputRange: [0, 0.45] }) },
        ]}
      />
    </>
  );
}

/** A heartbeat scale loop (1 → 1.12 → 1 → 1.08 → 1, pause). */
function useBeat(on: boolean) {
  const v = useRef(new Animated.Value(1)).current;
  useEffect(() => {
    if (!on) return;
    const to = (x: number, d: number) => Animated.timing(v, { toValue: x, duration: d, useNativeDriver: true });
    const anim = Animated.loop(Animated.sequence([to(1.12, 120), to(1, 120), to(1.08, 120), to(1, 160), Animated.delay(420)]));
    anim.start();
    return () => anim.stop();
  }, [v, on]);
  return v;
}

/** Keeps something shaking (devil) or glitching (404) for as long as it is on screen. */
function useJitter(kind: 'shake' | 'glitch' | null) {
  const v = useRef(new Animated.Value(0)).current;
  useEffect(() => {
    if (!kind) return;
    const steps = kind === 'shake' ? [6, -6, 4, -4, 0] : [0, 8, -5, 0, 0, 3, -9, 0];
    const anim = Animated.loop(
      Animated.sequence(
        steps.map((to) => Animated.timing(v, { toValue: to, duration: kind === 'shake' ? 60 : 70, useNativeDriver: true })),
      ),
    );
    anim.start();
    return () => anim.stop();
  }, [v, kind]);
  return v;
}

/** Full-screen party for a milestone. Tap anywhere (or wait) to close. */
export function Celebration({ count, onDone }: { count: number; onDone: () => void }) {
  const t = useTheme();
  const m = describe(count);
  const l = look(m, t);
  const clock = useClock(l.duration);
  const pop = useRef(new Animated.Value(0)).current;
  const fade = useRef(new Animated.Value(0)).current;
  const beat = useBeat(m.theme === 'hearts');
  const jitter = useJitter(m.theme === 'devil' ? 'shake' : m.theme === 'glitch' ? 'glitch' : null);
  const size = String(count).length > 3 ? 96 : 120;

  useEffect(() => {
    Animated.timing(fade, { toValue: 1, duration: 200, useNativeDriver: true }).start();
    Animated.sequence([
      Animated.delay(m.theme === 'gold' ? 300 : 150),
      Animated.spring(pop, { toValue: 1, friction: m.theme === 'gold' ? 5 : 4, tension: 60, useNativeDriver: true }),
    ]).start();
    const timer = setTimeout(close, l.duration + 1200);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function close() {
    Animated.timing(fade, { toValue: 0, duration: 250, useNativeDriver: true }).start(() => onDone());
  }

  // Gold slams in from big; the others pop up from small.
  const scale = pop.interpolate({ inputRange: [0, 1], outputRange: [m.theme === 'gold' ? 2.6 : 0.3, 1] });
  const number =
    m.theme === 'jackpot' ? (
      <Reels value={count} size={size * 0.8} color={l.number} />
    ) : (
      <View>
        {m.theme === 'glitch' && (
          <>
            <Animated.Text
              style={[
                styles.ghost,
                { fontSize: size, lineHeight: size * 1.04, color: '#FF2A6D', transform: [{ translateX: Animated.add(jitter, 4) }] },
              ]}>
              {count}
            </Animated.Text>
            <Animated.Text
              style={[
                styles.ghost,
                {
                  fontSize: size,
                  lineHeight: size * 1.04,
                  color: '#05D9E8',
                  transform: [{ translateX: Animated.add(Animated.multiply(jitter, -1), -4) }],
                },
              ]}>
              {count}
            </Animated.Text>
          </>
        )}
        <Animated.Text
          style={{
            fontFamily: t.font.display,
            fontSize: size,
            lineHeight: size * 1.04,
            letterSpacing: -4,
            color: l.number,
            textShadowColor:
              m.theme === 'devil'
                ? '#FF0000'
                : m.theme === 'matrix'
                  ? '#3DFF6E'
                  : m.theme === 'pixel'
                    ? '#FF2A6D'
                    : m.theme === 'night'
                      ? '#FFE9A8'
                      : 'transparent',
            textShadowRadius: m.theme === 'devil' || m.theme === 'matrix' || m.theme === 'night' ? 24 : 0,
            textShadowOffset: m.theme === 'pixel' ? { width: 6, height: 6 } : { width: 0, height: 0 },
            transform: [{ translateX: m.theme === 'devil' ? jitter : 0 }, { scale: beat }],
          }}>
          {count}
        </Animated.Text>
      </View>
    );

  return (
    <Animated.View style={[StyleSheet.absoluteFill, { opacity: fade, zIndex: 100 }]}>
      <Pressable accessibilityRole="button" accessibilityLabel="Fermer" onPress={close} style={StyleSheet.absoluteFill}>
        <View style={[StyleSheet.absoluteFill, { backgroundColor: l.bg, opacity: l.bg === t.bg ? 0.95 : 0.97 }]} />
        {m.theme === 'confetti' && <Rain clock={clock} colors={CONFETTI} count={70} />}
        {m.theme === 'emoji' && <Rain clock={clock} colors={CONFETTI} emoji={m.emoji} count={34} />}
        {m.theme === 'gold' && (
          <>
            <Fireworks clock={clock} colors={GOLD} bursts={m.epic ? 6 : 3} />
            <Rain clock={clock} colors={GOLD} count={m.epic ? 110 : 70} />
          </>
        )}
        {m.theme === 'devil' && <Rain clock={clock} colors={['#FF2A2A']} emoji={['🔥', '🔥', '😈', '🔥']} count={46} rise />}
        {m.theme === 'jackpot' && (
          <>
            <Rain clock={clock} colors={GOLD} emoji={m.emoji} count={40} />
            <Fireworks clock={clock} colors={['#FF4FA3', '#FFD54A', '#5AC8FA']} bursts={4} />
          </>
        )}
        {m.theme === 'matrix' && <MatrixRain clock={clock} />}
        {m.theme === 'rocket' && (
          <>
            <Stars />
            <Rocket clock={clock} />
          </>
        )}
        {m.theme === 'hearts' && <Rain clock={clock} colors={['#FF4FA3']} emoji={m.emoji} count={36} rise />}
        {m.theme === 'pixel' && <PixelRain clock={clock} />}
        {m.theme === 'siren' && (
          <>
            <SirenLights />
            <Rain clock={clock} colors={['#FF1E3C']} emoji={m.emoji} count={22} />
          </>
        )}
        {m.theme === 'tricolore' && (
          <>
            <Fireworks clock={clock} colors={TRICOLORE} bursts={6} />
            <Rain clock={clock} colors={TRICOLORE} count={90} />
          </>
        )}
        {m.theme === 'night' && (
          <>
            <Stars />
            <Rain clock={clock} colors={['#FFE9A8']} emoji={['🌙', '✨', '⭐']} count={16} />
          </>
        )}
        <View style={[StyleSheet.absoluteFill, { alignItems: 'center', justifyContent: 'center', padding: 32 }]} pointerEvents="none">
          {(m.theme === 'gold' || m.theme === 'jackpot') && <Rays color={l.number} />}
          <Animated.View
            style={{
              alignItems: 'center',
              gap: 10,
              transform: [{ scale }],
              opacity: pop.interpolate({ inputRange: [0, 0.3, 1], outputRange: [0, 1, 1] }),
            }}>
            {number}
            <Text style={{ fontFamily: t.font.display, fontSize: 28, letterSpacing: -0.5, color: l.text ?? t.txt }}>questions !</Text>
            <Text
              style={{
                fontFamily: t.font.semibold,
                fontSize: 16,
                lineHeight: 22,
                color: l.text ?? t.mut,
                opacity: l.text ? 0.85 : 1,
                textAlign: 'center',
                maxWidth: 290,
              }}>
              {m.line}
            </Text>
          </Animated.View>
        </View>
      </Pressable>
    </Animated.View>
  );
}

const styles = StyleSheet.create({
  ghost: { position: 'absolute', left: 0, top: 0, fontFamily: 'BricolageGrotesque_800ExtraBold', letterSpacing: -4, opacity: 0.8 },
});
