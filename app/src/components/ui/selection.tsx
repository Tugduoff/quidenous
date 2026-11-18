import { useFocusEffect } from 'expo-router';
import { Check, ListChecks, type LucideIcon, X } from 'lucide-react-native';
import { type ReactNode, useCallback, useState } from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import Animated, { FadeInDown, FadeOutDown, ReduceMotion, ZoomIn } from 'react-native-reanimated';
import { useT } from '@/lib/i18n';
import { LAYOUT_EASE, motion, usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { askConfirm } from '@/lib/confirm';
import { impactHaptic, selectionHaptic } from '@/lib/device/haptics';
import { useHardwareBack } from '@/lib/hooks/use-hardware-back';

const REDUCE = ReduceMotion.System;

const tick = () => {
  selectionHaptic();
};
const thud = () => {
  impactHaptic();
};

export type Selection<K> = {
  on: boolean;
  selected: K[];
  count: number;
  has: (id: K) => boolean;
  start: (id?: K) => void;
  toggle: (id: K) => void;
  all: () => void;
  allOn: boolean;
  cancel: () => void;
};

export function useSelection<K extends string | number>(ids: readonly K[] | undefined, enabled = true): Selection<K> {
  const [started, setOn] = useState(false);
  const [picked, setPicked] = useState<ReadonlySet<K>>(() => new Set());
  if (!enabled && (started || picked.size)) {
    setOn(false);
    setPicked(new Set());
  }
  const total = ids?.length ?? 0;
  const on = started && total > 0;
  const selected = on ? (ids ?? []).filter((id) => picked.has(id)) : [];
  const ticked = new Set(selected);
  const cancel = useCallback(() => {
    setOn(false);
    setPicked(new Set());
  }, []);
  useHardwareBack(cancel, on);
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

function Checkbox({ checked, disabled }: { checked: boolean; disabled?: boolean }) {
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

export function SelectableRow<K extends string | number>({
  sel,
  id,
  label,
  disabled,
  children,
}: {
  sel: Selection<K>;
  id: K;
  label: string;
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
  tone?: 'danger' | 'acc';
  disabled?: boolean;
  busy?: boolean;
};

export function SelectionBar<K>({ sel, actions }: { sel: Selection<K>; actions: BatchAction[] }) {
  const t = useTheme();
  if (!sel.on) return null;
  const none = sel.count === 0;
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

export const confirmBatch = (title: string, action: string, message?: string) => askConfirm(title, message, action, 'danger');

export async function eachOf<K>(ids: K[], one: (id: K) => Promise<unknown>, parallel = 3): Promise<K[]> {
  const done: K[] = [];
  let next = 0;
  const worker = async () => {
    while (next < ids.length) {
      const id = ids[next++];
      try {
        await one(id);
        done.push(id);
      } catch {}
    }
  };
  await Promise.all(Array.from({ length: Math.min(parallel, ids.length) }, worker));
  return ids.filter((id) => done.includes(id));
}
