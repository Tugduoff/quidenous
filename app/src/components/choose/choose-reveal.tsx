import { ArrowRight, Check, List } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated, { FadeIn, ReduceMotion, useSharedValue, withTiming, ZoomIn } from 'react-native-reanimated';
import Svg from 'react-native-svg';
import { Verdict } from '@/components/choose/verdict';
import { FlyingWire } from '@/components/choose/wires';
import { Avatar, AvatarStack } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { PressableScale } from '@/components/ui/pressable-scale';
import type { Member } from '@/lib/api/group';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';
import { useT } from '@/lib/i18n';
import { EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { type ChoosePerson as Person, measured } from '@/lib/utils/choose';
import { firstName } from '@/lib/utils/names';

const sys = ReduceMotion.System;

const WIRE_MS = 520;

/** One person at a time: the wire flies to what the group guessed, then their real choice lights up. */
export function ChooseReveal({ people, items, byId, onAll }: { people: Person[]; items: string[]; byId: Map<number, Member>; onAll: () => void }) {
  const t = useTheme();
  const tr = useT();
  const [step, setStep] = useState(0);
  const p = people[Math.min(step, people.length - 1)];
  if (!p) return null;
  const last = step >= people.length - 1;
  return (
    <Card style={{ padding: 16, gap: 14 }}>
      <RevealStage key={p.member_id} p={p} items={items} byId={byId} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>
          {step + 1}/{people.length}
        </Text>
        <View style={{ flex: 1 }} />
        {!last && (
          <Pressable accessibilityRole="button" onPress={onAll} hitSlop={8} style={{ minHeight: 40, justifyContent: 'center', paddingHorizontal: 6 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('results.choose.all')}</Text>
          </Pressable>
        )}
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr(last ? 'results.choose.all' : 'results.choose.next')}
          onPress={() => (last ? onAll() : setStep(step + 1))}
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 6,
            height: 40,
            paddingHorizontal: 16,
            borderRadius: t.radius.pill,
            backgroundColor: t.card2,
          }}>
          {last && <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>{tr('results.choose.all')}</Text>}
          {last ? <List size={16} color={t.txt} /> : <ArrowRight size={16} color={t.txt} />}
        </PressableScale>
      </View>
    </Card>
  );
}

type Box = { x: number; y: number; w: number; h: number };
const boxOf = (l: { x: number; y: number; width: number; height: number }): Box => ({ x: l.x, y: l.y, w: l.width, h: l.height });

function RevealStage({ p, items, byId }: { p: Person; items: string[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const m = byId.get(p.member_id);
  const color = m?.color ?? t.mut;
  const [size, setSize] = useState({ w: 0, h: 0 });
  // The face inside its column (name under it), the column inside the row.
  const [face, setFace] = useState<Box | null>(null);
  const [who, setWho] = useState<Box | null>(null);
  const [col, setCol] = useState<Box | null>(null);
  const [rows, setRows] = useState<(Box | undefined)[]>([]);
  const measuredRows = measured(rows, items.length);
  const ready = !!face && !!who && !!col && !!measuredRows;
  // The wire(s) fly out once everything is measured; the real choice lights up after they land.
  const fly = useSharedValue(0);
  useAnimateOnChange(fly, ready, () => (ready ? withTiming(1, { duration: WIRE_MS, easing: EASE, reduceMotion: sys }) : undefined));
  const from = face && who ? { x: who.x + face.x + face.w + 2, y: who.y + face.y + face.h / 2 } : { x: 0, y: 0 };
  return (
    <View style={{ gap: 14 }}>
      <View
        onLayout={(e) => setSize({ w: e.nativeEvent.layout.width, h: e.nativeEvent.layout.height })}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 48 }}>
        <View style={{ width: 76, alignItems: 'center', gap: 6 }} onLayout={(e) => setWho(boxOf(e.nativeEvent.layout))}>
          <View onLayout={(e) => setFace(boxOf(e.nativeEvent.layout))}>
            <Avatar member={m} size={56} />
          </View>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
            {m ? firstName(m.name) : '?'}
          </Text>
        </View>
        <View style={{ flex: 1, gap: 8 }} onLayout={(e) => setCol(boxOf(e.nativeEvent.layout))}>
          {items.map((label, k) => {
            const voters = p.guesses.filter((g) => g.item === k).map((g) => byId.get(g.voter_id));
            const real = p.real === k;
            return (
              <View
                key={k}
                onLayout={(e) => {
                  const b = boxOf(e.nativeEvent.layout);
                  setRows((rs) => Object.assign([...rs], { [k]: b }));
                }}
                style={{
                  minHeight: 44,
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 8,
                  paddingVertical: 8,
                  paddingHorizontal: 10,
                  borderRadius: 14,
                  backgroundColor: t.card2,
                }}>
                <Text numberOfLines={2} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 14, lineHeight: 18, color: t.txt }}>
                  {label}
                </Text>
                {voters.length > 0 && <AvatarStack members={voters} size={18} ring={t.card2} max={3} />}
                {/* Their own wire. */}
                {real && (
                  <Animated.View
                    entering={FadeIn.delay(WIRE_MS + 120)
                      .duration(220)
                      .reduceMotion(sys)}
                    pointerEvents="none"
                    style={{ position: 'absolute', left: -2, top: -2, right: -2, bottom: -2, borderRadius: 16, borderWidth: 2, borderColor: color }}
                  />
                )}
                {real && (
                  <Animated.View
                    entering={ZoomIn.delay(WIRE_MS + 120)
                      .springify()
                      .damping(12)
                      .reduceMotion(sys)}
                    style={{ width: 20, height: 20, borderRadius: 10, alignItems: 'center', justifyContent: 'center', backgroundColor: color }}>
                    <Check size={13} color="#121216" strokeWidth={3} />
                  </Animated.View>
                )}
              </View>
            );
          })}
        </View>
        {ready && col && measuredRows && size.w > 0 && (
          <View pointerEvents="none" style={{ position: 'absolute', left: 0, top: 0, width: size.w, height: size.h }}>
            <Svg width={size.w} height={size.h}>
              {p.top.map((k) => {
                const r = measuredRows[k];
                return <FlyingWire key={k} color={color} from={from} to={{ x: col.x - 2, y: col.y + r.y + r.h / 2 }} fly={fly} />;
              })}
            </Svg>
          </View>
        )}
      </View>
      <Animated.View
        entering={FadeIn.delay(WIRE_MS + 220)
          .duration(220)
          .reduceMotion(sys)}>
        <Verdict p={p} />
      </Animated.View>
    </View>
  );
}
