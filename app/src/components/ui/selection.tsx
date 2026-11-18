import { useFocusEffect } from 'expo-router';
import { Check, ListChecks, type LucideIcon, X } from 'lucide-react-native';
import { type ReactNode, useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, BackHandler, Platform, Pressable, Text, View } from 'react-native';
import Animated, { FadeInDown, FadeOutDown, ReduceMotion, ZoomIn } from 'react-native-reanimated';
import { useT } from '@/lib/i18n';
import { LAYOUT_EASE, motion, usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { askConfirm } from '@/lib/confirm';
import { impactHaptic, selectionHaptic } from '@/lib/device/haptics';

// Acting on several items of a list at once, the same way on every list:
// hold an item (or tap the header's "Sélectionner", the only way in on lists whose hold already means
// something, like dragging the question queue), tick the others, then pick an action in the bar at the bottom.
// The header shows the count, "Tout sélectionner" and a cross to leave; Android's back button and
// leaving the screen leave too.

const REDUCE = ReduceMotion.System;

const tick = () => {
  selectionHaptic();
};
const thud = () => {
  impactHaptic();
};

export type Selection<K> = {
  /** Selecting: rows show their checkbox, a tap ticks instead of opening. */
  on: boolean;
  /** The ticked ids, in the list's order (ids gone from the list drop out on their own). */
  selected: K[];
  count: number;
  has: (id: K) => boolean;
  /** Start selecting, with `id` ticked (a hold on a row) or nothing (the header's button). */
  start: (id?: K) => void;
  toggle: (id: K) => void;
  /** Tick everything, or nothing when everything already is. */
  all: () => void;
  allOn: boolean;
  cancel: () => void;
};

/** `ids`: the items that can be selected, in the list's order. */
export function useSelection<K extends string | number>(ids: readonly K[] | undefined): Selection<K> {
  const [started, setOn] = useState(false);
  const [picked, setPicked] = useState<ReadonlySet<K>>(() => new Set());
  const total = ids?.length ?? 0;
  // Nothing left to pick (the last ones were just deleted): back to the plain list.
  const on = started && total > 0;
  const selected = on ? (ids ?? []).filter((id) => picked.has(id)) : [];
  const ticked = new Set(selected);
  const cancel = useCallback(() => {
    setOn(false);
    setPicked(new Set());
  }, []);
  // Android's back button leaves the selection before the screen.
  useEffect(() => {
    if (!on || Platform.OS !== 'android') return;
    const sub = BackHandler.addEventListener('hardwareBackPress', () => {
      cancel();
      return true;
    });
    return () => sub.remove();
  }, [on, cancel]);
  // Another tab or screen: coming back shows the plain list.
  useFocusEffect(useCallback(() => cancel, [cancel]));
  return {
    on,
    selected,
    count: selected.length,
    has: (id) => ticked.has(id),
    start: (id) => {
      thud();
      setOn(true);
      setPicked(new Set(id === undefined ? [] : [id]));
    },
    toggle: (id) => {
      tick();
      setPicked((p) => {
        const next = new Set(p);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      });
    },
    all: () => {
      tick();
      setPicked(selected.length === total ? new Set() : new Set(ids));
    },
    allOn: total > 0 && selected.length === total,
    cancel,
  };
}

/** The header's way in (the web has no hold): a round button like the page headers'. */
export function SelectButton({ onPress, size = 40 }: { onPress: () => void; size?: number }) {
  const t = useTheme();
  const tr = useT();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={tr('common.select.start')}
      onPress={() => onPress()}
      hitSlop={4}
      style={{ width: size, height: size, borderRadius: size / 2, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
      <ListChecks size={size >= 40 ? 19 : 17} color={t.txt} />
    </PressableScale>
  );
}

/** Takes the place of the list's header while selecting: leave, how many, all / none. */
export function SelectionHeader<K>({ sel }: { sel: Selection<K> }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View entering={motion.enter} style={{ flexDirection: 'row', alignItems: 'center', gap: 10, minHeight: 40 }}>
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={tr('common.select.cancel')}
        onPress={sel.cancel}
        hitSlop={4}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <X size={20} color={t.txt} />
      </PressableScale>
      <Text accessibilityLiveRegion="polite" numberOfLines={1} style={{ flex: 1, fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
        {tr('common.select.count', { count: sel.count })}
      </Text>
      <Pressable accessibilityRole="button" onPress={sel.all} hitSlop={8}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>{sel.allOn ? tr('common.select.none') : tr('common.select.all')}</Text>
      </Pressable>
    </Animated.View>
  );
}

/** The import list's tick box, shared: filled with the accent and a check when on. */
export function Checkbox({ checked, disabled }: { checked: boolean; disabled?: boolean }) {
  const t = useTheme();
  const pop = usePop(checked, 1.15, checked);
  return (
    <Animated.View
      style={[
        {
          width: 22,
          height: 22,
          borderRadius: 7,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: checked ? t.acc : 'transparent',
          borderWidth: checked ? 0 : 2,
          borderColor: t.line,
          opacity: disabled ? 0.5 : 1,
        },
        pop,
      ]}>
      {checked && <Check size={14} color={t.accTxt} strokeWidth={3} />}
    </Animated.View>
  );
}

/**
 * A row of a list while selecting: its checkbox on the left, a tap anywhere ticks it, and what's inside
 * (the row's own buttons and links) stays still. Outside selection it is just `children`.
 */
export function SelectableRow<K extends string | number>({
  sel,
  id,
  label,
  disabled,
  children,
}: {
  sel: Selection<K>;
  id: K;
  /** What a screen reader says for it. */
  label: string;
  /** Shown but not selectable (someone else's question when I can only delete). */
  disabled?: boolean;
  children: ReactNode;
}) {
  const tr = useT();
  if (!sel.on) return <>{children}</>;
  const checked = sel.has(id);
  return (
    <Pressable
      accessibilityRole="checkbox"
      accessibilityLabel={tr('common.select.item', { name: label })}
      accessibilityState={{ checked, disabled: !!disabled }}
      disabled={disabled}
      onPress={() => sel.toggle(id)}
      style={{ flexDirection: 'row', alignItems: 'center', gap: 12, opacity: disabled ? 0.5 : 1 }}>
      <Animated.View entering={ZoomIn.duration(160).reduceMotion(REDUCE)}>
        <Checkbox checked={checked} disabled={disabled} />
      </Animated.View>
      <View pointerEvents="none" style={{ flex: 1, minWidth: 0 }}>
        {children}
      </View>
    </Pressable>
  );
}

export type BatchAction = {
  icon: LucideIcon;
  label: string;
  onPress: () => void;
  /** `danger`: deletes (pink); `acc`: the main go-ahead. */
  tone?: 'danger' | 'acc';
  disabled?: boolean;
  busy?: boolean;
};

/** The actions, at the bottom of the screen while selecting (hand it to the Screen's `footer`). */
export function SelectionBar<K>({ sel, actions }: { sel: Selection<K>; actions: BatchAction[] }) {
  const t = useTheme();
  if (!sel.on) return null;
  const none = sel.count === 0;
  // Three or more side by side don't fit a phone's width with their labels in line: icon above the label.
  const compact = actions.length > 2;
  return (
    <Animated.View
      entering={FadeInDown.duration(200).easing(LAYOUT_EASE).reduceMotion(REDUCE)}
      exiting={FadeOutDown.duration(150).reduceMotion(REDUCE)}
      style={{ flexDirection: 'row', gap: 8, padding: 6, borderRadius: compact ? 28 : t.radius.pill, backgroundColor: t.card, borderWidth: 1, borderColor: t.line }}>
      {actions.map((a) => {
        const off = none || !!a.disabled || !!a.busy;
        const acc = a.tone === 'acc';
        const ink = acc ? t.accTxt : a.tone === 'danger' ? t.tagInk : t.txt;
        return (
          <PressableScale
            key={a.label}
            accessibilityRole="button"
            accessibilityLabel={a.label}
            accessibilityState={{ disabled: off, busy: !!a.busy }}
            disabled={off}
            onPress={a.onPress}
            style={{
              flex: 1,
              minWidth: 0,
              height: compact ? 52 : 44,
              borderRadius: compact ? 22 : t.radius.pill,
              flexDirection: compact ? 'column' : 'row',
              alignItems: 'center',
              justifyContent: 'center',
              gap: compact ? 2 : 6,
              paddingHorizontal: compact ? 4 : 10,
              backgroundColor: acc ? t.acc : t.card2,
              opacity: off && !a.busy ? 0.45 : 1,
            }}>
            {a.busy ? <ActivityIndicator size="small" color={ink} /> : <a.icon size={17} color={ink} />}
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: compact ? 12 : 14, color: ink }}>
              {a.label}
            </Text>
          </PressableScale>
        );
      })}
    </Animated.View>
  );
}

/** One confirmation for a destructive batch, with the count in its title ("Supprimer 5 packs ?"). */
export const confirmBatch = (title: string, action: string, message?: string) => askConfirm(title, message, action, 'danger');

/**
 * Runs `one` on each id, a few at a time, for lists whose server has no batch endpoint;
 * resolves with the ids done (the others failed, the caller says how many).
 */
export async function eachOf<K>(ids: K[], one: (id: K) => Promise<unknown>, parallel = 3): Promise<K[]> {
  const done: K[] = [];
  let next = 0;
  const worker = async () => {
    while (next < ids.length) {
      const id = ids[next++];
      try {
        await one(id);
        done.push(id);
      } catch {
        // Counted by the caller as not done.
      }
    }
  };
  await Promise.all(Array.from({ length: Math.min(parallel, ids.length) }, worker));
  return ids.filter((id) => done.includes(id));
}
