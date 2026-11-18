// The first-run intro: la bande shows a new account how a round goes, then on to a first room (lib/intro.ts for the rules,
// docs/TODAY.md "Intro"). Four short steps: hello, a practice vote on the Petons, its verdict, your turn. Skippable at any time.
import { Crown } from 'lucide-react-native';
import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react';
import { BackHandler, Pressable, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion, withSpring, withTiming } from 'react-native-reanimated';

import { Mascot } from '@/components/mascot';
import { MascotBand } from '@/components/mascot-band';
import { Fill, Land, Landed } from '@/components/result-bars';
import { Screen } from '@/components/screen';
import { Avatar, Button, Card, Display, Logo, PressableScale, QuestionText } from '@/components/ui';
import { useT } from '@/lib/i18n';
import { DEMO_ORDER, demoTally, type DemoRow } from '@/lib/intro';
import { CAST, MASCOT, type Who } from '@/lib/mascot';
import { BOUNCE, EASE, motion } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

type Step = 'hello' | 'vote' | 'verdict' | 'end';
const STEPS: Step[] = ['hello', 'vote', 'verdict', 'end'];
/** Between two of the band's votes coming in, once I voted. */
const VOTE_MS = 380;

export default function Intro() {
  const tr = useT();
  const { endIntro, roomless } = useSession();
  const [step, setStep] = useState<Step>('hello');
  const [pick, setPick] = useState<Who | null>(null);
  const i = STEPS.indexOf(step);
  const counted = useCallback(() => setStep('verdict'), []);

  // Back a step (Android's button; the first step leaves the app, as any first screen). Back to the practice vote
  // (from its verdict or the end): a fresh ballot.
  useEffect(() => {
    if (i === 0) return;
    const sub = BackHandler.addEventListener('hardwareBackPress', () => {
      const prev = STEPS[i - 1] === 'verdict' ? 'vote' : STEPS[i - 1];
      if (prev === 'vote') setPick(null);
      setStep(prev);
      return true;
    });
    return () => sub.remove();
  }, [i]);

  const footer =
    step === 'hello' ? (
      <Button testID="intro-start" onPress={() => setStep('vote')}>
        {tr('intro.hello.cta')}
      </Button>
    ) : step === 'verdict' ? (
      <Button testID="intro-next" onPress={() => setStep('end')}>
        {tr('intro.next')}
      </Button>
    ) : step === 'end' ? (
      <Button testID="intro-done" onPress={() => endIntro(false)}>
        {tr(roomless ? 'intro.end.cta' : 'intro.end.inCta')}
      </Button>
    ) : undefined;

  return (
    <Screen tabs={false} footer={footer}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', minHeight: 44 }}>
        <Dots index={i} count={STEPS.length} />
        {step !== 'end' && (
          <Pressable testID="intro-skip" accessibilityRole="button" onPress={() => endIntro(true)} hitSlop={10} style={{ padding: 8 }}>
            <SkipText>{tr('intro.skip')}</SkipText>
          </Pressable>
        )}
      </View>
      <Animated.View key={step} entering={motion.question} style={{ flex: 1, gap: 20 }}>
        {step === 'hello' && <Hello />}
        {step === 'vote' && (
<Vote pick={pick} onPick={setPick} onCounted={counted} />
        )}
        {step === 'verdict' && pick && <Verdict pick={pick} />}
        {step === 'end' && <End roomless={roomless} />}
      </Animated.View>
    </Screen>
  );
}

function SkipText({ children }: { children: ReactNode }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{children}</Text>;
}

/** Where I am in the intro: one dot per step, the current one a short pill. */
function Dots({ index, count }: { index: number; count: number }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View accessible accessibilityLabel={tr('intro.step', { n: index + 1, total: count })} style={{ flexDirection: 'row', gap: 6 }}>
      {Array.from({ length: count }, (_, k) => (
        <Dot key={k} on={k === index} done={k < index} color={t.accInk} off={t.line} />
      ))}
    </View>
  );
}

function Dot({ on, done, color, off }: { on: boolean; done: boolean; color: string; off: string }) {
  const reduced = useReducedMotion();
  const style = useAnimatedStyle(() => ({
    width: reduced ? (on ? 20 : 8) : withSpring(on ? 20 : 8, BOUNCE),
    backgroundColor: on || done ? color : off,
  }));
  return <Animated.View style={[{ height: 8, borderRadius: 4 }, style]} />;
}

/** The whole band waves hello. */
function Hello() {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ flex: 1, justifyContent: 'center', gap: 24 }}>
      <Logo size={30} />
      <View style={{ gap: 12 }}>
        <Display size={34}>{tr('intro.hello.title')}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 17, lineHeight: 24, color: t.mut }}>{tr('intro.hello.body')}</Text>
      </View>
      <MascotBand hello max={460} style={{ marginTop: 8 }} />
    </View>
  );
}

/** The practice question: pick a Peton, then watch the band's votes come in. */
function Vote({ pick, onPick, onCounted }: { pick: Who | null; onPick: (w: Who) => void; onCounted: () => void }) {
  const t = useTheme();
  const tr = useT();
  const reduced = useReducedMotion();
  // Votes in: mine, then the band's one by one; all in, on to the verdict.
  const total = 1 + DEMO_ORDER.length;
  const [count, setCount] = useState(pick ? total : 0);
  const choose = (who: Who) => {
    onPick(who);
    setCount(1);
  };
  useEffect(() => {
    if (!pick || count === 0) return;
    const all = count >= total;
    const timer = setTimeout(all ? onCounted : () => setCount(count + 1), all ? (reduced ? 250 : 450) : reduced ? 0 : VOTE_MS);
    return () => clearTimeout(timer);
  }, [pick, count, total, reduced, onCounted]);

  return (
    <>
      <Card style={{ padding: 18, gap: 10 }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 12, letterSpacing: 0.6, color: t.accInk, textTransform: 'uppercase' }}>
          {tr('intro.vote.eyebrow')}
        </Text>
        <QuestionText text={tr('intro.vote.question')} rank={false} style={{ fontFamily: t.font.display, fontSize: 26, lineHeight: 29, letterSpacing: -0.6, color: t.txt }} />
      </Card>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10, justifyContent: 'center' }}>
        {CAST.map((who) => (
          <Candidate key={who} who={who} picked={pick === who} dim={!!pick && pick !== who} onPress={pick ? undefined : () => choose(who)} />
        ))}
      </View>
      <View style={{ alignItems: 'center', gap: 10, minHeight: 40 }} accessibilityLiveRegion="polite">
        {pick ? (
          <>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{tr('intro.vote.counting', { n: Math.max(1, count) })}</Text>
            <Progress value={count / (1 + DEMO_ORDER.length)} />
          </>
        ) : (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{tr('intro.vote.hint')}</Text>
        )}
      </View>
    </>
  );
}

function Candidate({ who, picked, dim, onPress }: { who: Who; picked: boolean; dim: boolean; onPress?: () => void }) {
  const t = useTheme();
  const tr = useT();
  const name = tr(`intro.names.${who}`);
  const style = useAnimatedStyle(() => ({ opacity: withTiming(dim ? 0.4 : 1, { duration: 200, easing: EASE }) }));
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={tr('intro.vote.pickA11y', { name })}
      accessibilityState={{ selected: picked, disabled: !onPress }}
      disabled={!onPress}
      onPress={onPress}
      style={{ width: '30%', minWidth: 96, maxWidth: 140 }}>
      <Animated.View
        style={[
          { alignItems: 'center', gap: 4, paddingTop: 8, paddingBottom: 10, borderRadius: 18, backgroundColor: t.card, borderWidth: 2, borderColor: picked ? t.accInk : 'transparent' },
          t.shadow,
          style,
        ]}>
        <Mascot who={who} mood={picked ? 'celebrate' : 'happy'} size={72} animate={picked} />
        <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>{name}</Text>
      </Animated.View>
    </PressableScale>
  );
}

function Progress({ value }: { value: number }) {
  const t = useTheme();
  const style = useAnimatedStyle(() => ({ transform: [{ scaleX: withTiming(value, { duration: 260, easing: EASE }) }] }));
  return (
    <View style={{ width: 200, height: 8, borderRadius: 4, backgroundColor: t.card2, overflow: 'hidden' }}>
      <Animated.View style={[{ width: '100%', height: 8, borderRadius: 4, backgroundColor: t.accInk, transformOrigin: 'left' }, style]} />
    </View>
  );
}

/** The result: Bulle, as always, caught. */
function Verdict({ pick }: { pick: Who }) {
  const t = useTheme();
  const tr = useT();
  const rows = useMemo(() => demoTally(pick), [pick]);
  const right = rows[0]?.who === pick;
  return (
    <>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <View style={{ flex: 1, gap: 8 }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 12, letterSpacing: 0.6, color: t.accInk, textTransform: 'uppercase' }}>
            {tr('intro.verdict.eyebrow')}
          </Text>
          <Display size={28}>{tr(right ? 'intro.verdict.right' : 'intro.verdict.wrong')}</Display>
        </View>
        <Mascot who="bulle" mood="caught" size={104} />
      </View>
      <QuestionText text={tr('intro.vote.question')} rank={false} style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 21, color: t.mut }} />
      <View style={{ gap: 10 }}>
        {rows.map((r, i) => (
          <DemoBar key={r.who} row={r} index={i} rows={rows.length} />
        ))}
      </View>
      <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{tr('intro.verdict.body')}</Text>
    </>
  );
}

/** A result row as the real ones look (components/result-bars.tsx), with the Petons as the people. */
function DemoBar({ row: r, index, rows }: { row: DemoRow; index: number; rows: number }) {
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
        {/* Who voted for them: me (my face) and the band (their colours). */}
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

/** On to the real thing: Zeste, the instigator, sends you off. */
function End({ roomless }: { roomless: boolean }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ flex: 1, justifyContent: 'center', alignItems: 'center', gap: 16 }}>
      <Mascot who="zeste" mood="celebrate" size={160} />
      <View style={{ gap: 10, alignSelf: 'stretch' }}>
        <Display size={34}>{tr(roomless ? 'intro.end.title' : 'intro.end.inTitle')}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 17, lineHeight: 24, color: t.mut }}>{tr(roomless ? 'intro.end.body' : 'intro.end.inBody')}</Text>
      </View>
    </View>
  );
}
