import { useEffect, useState } from 'react';
import { ScrollView, StyleSheet, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion, useSharedValue, withDelay, withSequence, withSpring, withTiming } from 'react-native-reanimated';
import { useT } from '@/lib/i18n';
import { BOUNCE, EASE, usePop } from '@/lib/motion';
import { CHAT_REACTIONS, HEART, type Reaction } from '@/lib/api/chat';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Dialog } from '@/components/ui/dialog';
import { useMembers } from '@/lib/api/group';

/** The six reactions, on top of a message's long-press menu; mine are highlighted. */
export function ReactionBar({ reactions, onPick }: { reactions?: Reaction[]; onPick: (emoji: string) => void }) {
  const t = useTheme();
  const tr = useT();
  const mine = new Set((reactions ?? []).filter((r) => r.mine).map((r) => r.emoji));
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingBottom: 2 }}>
      {CHAT_REACTIONS.map((emoji) => {
        const on = mine.has(emoji);
        return (
          <PressableScale
            key={emoji}
            scale={0.85}
            accessibilityRole="button"
            accessibilityLabel={tr('chat.reactions.react', { emoji })}
            accessibilityState={{ selected: on }}
            onPress={() => onPick(emoji)}
            style={{
              flex: 1,
              maxWidth: 46,
              height: 46,
              borderRadius: 23,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: on ? t.accSoft : 'transparent',
            }}>
            <Text style={{ fontSize: 26 }}>{emoji}</Text>
          </PressableScale>
        );
      })}
    </View>
  );
}

/** The reactions under a message, on its side. Tap: mine on or off; long-press: who reacted. */
export function ReactionChips({
  reactions,
  mine,
  onToggle,
  onWho,
}: {
  reactions: Reaction[];
  /** My own message: the chips go right, under it. */
  mine: boolean;
  onToggle?: (emoji: string) => void;
  onWho: (emoji: string) => void;
}) {
  return (
    <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 4, justifyContent: mine ? 'flex-end' : 'flex-start', marginLeft: mine ? 0 : 36 }}>
      {reactions.map((r) => (
        <Chip key={r.emoji} r={r} onToggle={onToggle} onWho={onWho} />
      ))}
    </View>
  );
}

function Chip({ r, onToggle, onWho }: { r: Reaction; onToggle?: (emoji: string) => void; onWho: (emoji: string) => void }) {
  const t = useTheme();
  const tr = useT();
  // Pops when it becomes mine.
  const pop = usePop(r.mine, 1.2, r.mine);
  return (
    <Animated.View style={pop}>
      <PressableScale
        scale={0.9}
        accessibilityRole="button"
        accessibilityLabel={tr(r.mine ? 'chat.reactions.chipMine' : 'chat.reactions.chip', { emoji: r.emoji, count: r.count })}
        accessibilityHint={tr('chat.reactions.chipHint')}
        accessibilityState={{ selected: r.mine }}
        onPress={onToggle && (() => onToggle(r.emoji))}
        onLongPress={() => onWho(r.emoji)}
        delayLongPress={400}
        style={{
          height: 26,
          paddingHorizontal: 8,
          borderRadius: 13,
          flexDirection: 'row',
          alignItems: 'center',
          gap: 4,
          borderWidth: 1.5,
          borderColor: r.mine ? t.accInk : 'transparent',
          backgroundColor: r.mine ? t.accSoft : t.card,
        }}>
        <Text style={{ fontSize: 14 }}>{r.emoji}</Text>
        <Text style={{ fontFamily: t.font.bold, fontSize: 12, color: r.mine ? t.accInk : t.mut }}>{r.count}</Text>
      </PressableScale>
    </Animated.View>
  );
}

/** Who reacted with one emoji, oldest first. Someone no longer in the room's list: a deleted account. */
export function WhoReacted({ reaction, onClose }: { reaction: Reaction | null; onClose: () => void }) {
  const t = useTheme();
  const tr = useT();
  const { byId } = useMembers();
  // The last one stays drawn while the dialog fades out.
  const [shown, setShown] = useState<Reaction | null>(null);
  if (reaction && reaction !== shown) setShown(reaction);
  return (
    <Dialog visible={!!reaction} onClose={onClose}>
      <View accessible style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <Text style={{ fontSize: 26 }}>{shown?.emoji}</Text>
        <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{tr('chat.reactions.count', { count: shown?.count ?? 0 })}</Text>
      </View>
      <ScrollView style={{ maxHeight: 360 }} contentContainerStyle={{ gap: 4 }}>
        {(shown?.members ?? []).map((id) => {
          const m = byId.get(id);
          return (
            <View key={id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, height: 44 }}>
              <Avatar member={m} size={32} />
              <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: m ? t.txt : t.mut }}>
                {m?.name ?? tr('common.deletedAccount')}
              </Text>
            </View>
          );
        })}
      </ScrollView>
    </Dialog>
  );
}

/** A heart that blooms over a double-tapped message and fades. Nothing under reduced motion (the chip still pops). */
export function HeartBurst({ trigger }: { trigger: number }) {
  const reduced = useReducedMotion();
  const scale = useSharedValue(0.4);
  const opacity = useSharedValue(0);
  useEffect(() => {
    if (!trigger || reduced) return;
    scale.set(withSequence(withTiming(0.4, { duration: 0 }), withSpring(1, BOUNCE), withTiming(1.12, { duration: 260, easing: EASE })));
    opacity.set(withSequence(withTiming(1, { duration: 0 }), withDelay(380, withTiming(0, { duration: 180, easing: EASE }))));
  }, [trigger, reduced, scale, opacity]);
  const style = useAnimatedStyle(() => ({ opacity: opacity.value, transform: [{ scale: scale.value }] }));
  // Not there before the first double tap; never read out (the chip says it).
  if (reduced || !trigger) return null;
  return (
    <Animated.View aria-hidden pointerEvents="none" style={[StyleSheet.absoluteFill, { alignItems: 'center', justifyContent: 'center' }, style]}>
      <Text style={{ fontSize: 34 }}>{HEART}</Text>
    </Animated.View>
  );
}
