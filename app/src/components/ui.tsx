import { type ReactNode, useEffect, useState, useSyncExternalStore } from 'react';
import { Trophy, type LucideIcon } from 'lucide-react-native';
import {
  ActivityIndicator,
  KeyboardAvoidingView,
  Modal,
  Platform,
  Pressable,
  type PressableProps,
  type StyleProp,
  StyleSheet,
  Text,
  View,
  type TextStyle,
  type ViewStyle,
} from 'react-native';
import Animated, {
  interpolateColor,
  LayoutAnimationConfig,
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated';
import { Image } from 'expo-image';

import { AvatarArt } from '@/components/avatar-art';
import { photoUrl, type Member } from '@/lib/api';
import { parseAvatar } from '@/lib/avatar';
import { EASE, motion, SPRING } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

/** A Pressable that dips a little while held: the app's press feedback (none under reduced motion). */
export function PressableScale({
  scale = 0.96,
  style,
  onPressIn,
  onPressOut,
  ...props
}: Omit<PressableProps, 'style'> & { style?: StyleProp<ViewStyle>; scale?: number }) {
  const reduced = useReducedMotion();
  const s = useSharedValue(1);
  const pressed = useAnimatedStyle(() => ({ transform: [{ scale: s.value }] }));
  return (
    <AnimatedPressable
      {...props}
      onPressIn={(e) => {
        if (!reduced) s.value = withTiming(scale, { duration: 90 });
        onPressIn?.(e);
      }}
      onPressOut={(e) => {
        s.value = withSpring(1, SPRING);
        onPressOut?.(e);
      }}
      style={[style, pressed]}
    />
  );
}

export function Avatar({ member, size = 40 }: { member?: Member; size?: number }) {
  const t = useTheme();
  const uri = member && photoUrl(member);
  // A drawn avatar the app knows how to draw (a newer one falls back to the initial).
  const avatar = !uri && parseAvatar(member?.avatar) ? member?.avatar : null;
  return (
    <View
      style={{
        width: size,
        height: size,
        borderRadius: size / 2,
        backgroundColor: member?.color ?? t.card2,
        alignItems: 'center',
        justifyContent: 'center',
        overflow: 'hidden',
      }}>
      {uri ? (
        <Image source={{ uri }} style={{ width: size, height: size }} />
      ) : avatar ? (
        <AvatarArt avatar={avatar} size={size} />
      ) : (
        <Text style={{ fontFamily: t.font.bold, fontSize: size * 0.42, color: '#121216' }}>{member?.name[0]?.toUpperCase() ?? '?'}</Text>
      )}
    </View>
  );
}

const stackOverlap = (size: number) => Math.round(size * 0.3);

/** Width of an AvatarStack of `count` members (each cell carries a 2 px ring). */
export function avatarStackWidth(count: number, size: number, max?: number) {
  const n = max ? Math.min(count, max) : count;
  return n * (size + 4) - (n - 1) * stackOverlap(size);
}

/** Overlapping avatars. `ring` should match the surface behind it; past `max`, a "+N" bubble. */
export function AvatarStack({ members, size = 28, ring, max }: { members: (Member | undefined)[]; size?: number; ring?: string; max?: number }) {
  const t = useTheme();
  const shown = max && members.length > max ? members.slice(0, max - 1) : members;
  const extra = members.length - shown.length;
  const overlap = stackOverlap(size);
  const cell = (key: string | number, i: number, child: ReactNode) => (
    <Animated.View
      key={key}
      entering={motion.pop}
      style={{ marginLeft: i ? -overlap : 0, borderRadius: size, borderWidth: 2, borderColor: ring ?? t.bg }}>
      {child}
    </Animated.View>
  );
  // Faces there from the start just show; one joining later (a new vote) pops in.
  return (
    <LayoutAnimationConfig skipEntering>
      <View style={{ flexDirection: 'row' }}>
        {shown.map((m, i) => cell(m?.id ?? i, i, <Avatar member={m} size={size} />))}
        {extra > 0 &&
          cell(
            'extra',
            shown.length,
            <View
              style={{
                width: size,
                height: size,
                borderRadius: size / 2,
                backgroundColor: t.mut,
                alignItems: 'center',
                justifyContent: 'center',
              }}>
              <Text style={{ fontFamily: t.font.bold, fontSize: size * 0.36, color: t.card }}>+{extra}</Text>
            </View>,
          )}
      </View>
    </LayoutAnimationConfig>
  );
}

export function Button({
  children,
  onPress,
  disabled,
  loading,
  style,
}: {
  children: ReactNode;
  onPress?: () => void;
  disabled?: boolean;
  loading?: boolean;
  style?: ViewStyle;
}) {
  const t = useTheme();
  const off = disabled || loading;
  return (
    <PressableScale
      accessibilityRole="button"
      disabled={off}
      onPress={onPress}
      style={[styles.button, { backgroundColor: off ? t.card2 : t.acc }, style]}>
      {loading ? (
        <ActivityIndicator color={t.mut} />
      ) : typeof children === 'string' ? (
        <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: off ? t.mut : t.accTxt }}>{children}</Text>
      ) : (
        children
      )}
    </PressableScale>
  );
}

export function Tag({ label, tone = 'tag' }: { label: string; tone?: 'tag' | 'acc' }) {
  const t = useTheme();
  return (
    <View style={[styles.tag, { backgroundColor: tone === 'acc' ? t.acc : t.tag }]}>
      <Text
        style={{
          fontFamily: t.font.bold,
          fontSize: 12,
          letterSpacing: 0.3,
          color: tone === 'acc' ? t.accTxt : '#fff',
          textTransform: 'uppercase',
        }}>
        {label}
      </Text>
    </View>
  );
}

export function Card({ children, style }: { children: ReactNode; style?: ViewStyle }) {
  const t = useTheme();
  return <View style={[{ backgroundColor: t.card, borderRadius: t.radius.card }, t.shadow, style]}>{children}</View>;
}

export function Display({ children, size = 34 }: { children: ReactNode; size?: number }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.display, fontSize: size, lineHeight: size * 1.08, letterSpacing: -1, color: t.txt }}>{children}</Text>;
}

/**
 * A code in big type (invite, friend code) that always stays on one line: the font shrinks to the width it gets.
 * Measured rather than `adjustsFontSizeToFit`, which the web build ignores.
 */
export function CodeText({ code, size = 34, center }: { code: string; size?: number; center?: boolean }) {
  const t = useTheme();
  const [width, setWidth] = useState(0);
  const spacing = code.length > 10 ? 1.5 : 4;
  // Bricolage ExtraBold capitals and digits are about 0.68 em wide.
  const fit = width ? Math.floor((width / code.length - spacing) / 0.68) : size;
  const fontSize = Math.max(14, Math.min(size, fit));
  return (
    <View style={{ alignSelf: 'stretch' }} onLayout={(e) => setWidth(e.nativeEvent.layout.width)}>
      <Text
        selectable
        numberOfLines={1}
        adjustsFontSizeToFit
        minimumFontScale={0.4}
        style={{ fontFamily: t.font.display, fontSize, lineHeight: size * 1.2, letterSpacing: spacing, color: t.txt, textAlign: center ? 'center' : 'left' }}>
        {code}
      </Text>
    </View>
  );
}

export function Logo({ size = 22 }: { size?: number }) {
  const t = useTheme();
  return (
    <Text style={{ fontFamily: t.font.display, fontSize: size, letterSpacing: -0.5, color: t.txt }}>
      qui de nous<Text style={{ color: t.accInk }}>?</Text>
    </Text>
  );
}

const styles = StyleSheet.create({
  button: { height: 58, borderRadius: 999, alignItems: 'center', justifyContent: 'center', flexDirection: 'row', gap: 8 },
  tag: { alignSelf: 'flex-start', borderRadius: 999, paddingHorizontal: 10, paddingVertical: 5 },
});

/** A small on/off switch with its label, drawn to match the app (lime when on). An empty label shows the switch alone (`a11y` names it). */
export function Toggle({
  label,
  value,
  onChange,
  icon: Icon,
  a11y,
}: {
  label: string;
  value: boolean;
  onChange: (v: boolean) => void;
  icon?: LucideIcon;
  a11y?: string;
}) {
  const t = useTheme();
  const on = useSharedValue(value ? 1 : 0);
  useEffect(() => {
    on.value = withSpring(value ? 1 : 0, SPRING);
  }, [value, on]);
  // Plain strings: the worklets below copy what they use, and the theme object carries more than colours.
  const { card2, acc, line, mut, accTxt } = t;
  const track = useAnimatedStyle(() => ({
    backgroundColor: interpolateColor(on.value, [0, 1], [card2, acc]),
    borderColor: interpolateColor(on.value, [0, 1], [line, acc]),
  }));
  // 40 wide, minus the 1 px border and 3 px padding on each side, minus the 16 px thumb.
  const thumb = useAnimatedStyle(() => ({
    transform: [{ translateX: on.value * 16 }],
    backgroundColor: interpolateColor(on.value, [0, 1], [mut, accTxt]),
  }));
  return (
    <Pressable
      accessibilityRole="switch"
      accessibilityLabel={a11y ?? label}
      accessibilityState={{ checked: value }}
      onPress={() => onChange(!value)}
      hitSlop={8}
      style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
      {Icon && <Icon size={14} color={value ? t.accInk : t.mut} style={{ marginRight: -3 }} />}
      {!!label && <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: value ? t.accInk : t.mut }}>{label}</Text>}
      <Animated.View style={[{ width: 40, height: 24, borderRadius: 12, padding: 3, borderWidth: 1, justifyContent: 'center' }, track]}>
        <Animated.View style={[{ width: 16, height: 16, borderRadius: 8 }, thumb]} />
      </Animated.View>
    </Pressable>
  );
}

/** A question's text; ranking questions get a trophy in front, lined up with the first line. */
export function QuestionText({
  text,
  rank,
  style,
  containerStyle,
}: {
  text: string;
  rank: boolean;
  style: TextStyle & { fontSize: number; lineHeight: number };
  containerStyle?: ViewStyle;
}) {
  const t = useTheme();
  const size = Math.round(style.fontSize * 0.95);
  return (
    <View style={[{ flexDirection: 'row', alignItems: 'flex-start', gap: 6, flexShrink: 1 }, containerStyle]}>
      {rank && <Trophy size={size} color={t.accInk} style={{ flexShrink: 0, marginTop: (style.lineHeight - size) / 2 }} />}
      <Text style={[{ flex: 1 }, style]}>{text}</Text>
    </View>
  );
}

/** The app's dialog: a card centred on a dark backdrop (tapping the backdrop closes it). Rises with the keyboard. */
export function Dialog({ visible, onClose, children }: { visible: boolean; onClose: () => void; children: ReactNode }) {
  const t = useTheme();
  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : undefined} style={{ flex: 1 }}>
        <Pressable
          accessibilityLabel="Fermer"
          onPress={onClose}
          style={{ flex: 1, backgroundColor: '#0008', alignItems: 'center', justifyContent: 'center', padding: 24 }}>
          {/* Taps inside the card don't close it. */}
          <ModalCard
            style={{
              width: '100%',
              maxWidth: 380,
              borderRadius: 24,
              padding: 22,
              gap: 12,
              backgroundColor: t.card,
              borderWidth: 1,
              borderColor: t.line,
            }}>
            {children}
          </ModalCard>
        </Pressable>
      </KeyboardAvoidingView>
    </Modal>
  );
}

/** Title and grey message of a Dialog. */
export function DialogTitle({ title, message }: { title: string; message?: string }) {
  const t = useTheme();
  return (
    <>
      <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{title}</Text>
      {!!message && <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{message}</Text>}
    </>
  );
}

/** Cancel and a red action, side by side, at the bottom of a Dialog. */
export function DialogActions({
  confirmLabel,
  onConfirm,
  onCancel,
  busy,
  disabled,
}: {
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  busy?: boolean;
  disabled?: boolean;
}) {
  const t = useTheme();
  const off = busy || disabled;
  return (
    <View style={{ flexDirection: 'row', gap: 10, marginTop: 6 }}>
      <PressableScale
        accessibilityRole="button"
        onPress={onCancel}
        style={{
          flex: 1,
          height: 46,
          borderRadius: t.radius.pill,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: t.card2,
        }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Annuler</Text>
      </PressableScale>
      <PressableScale
        accessibilityRole="button"
        accessibilityState={{ disabled: !!off }}
        disabled={off}
        onPress={onConfirm}
        style={{
          flex: 1,
          height: 46,
          borderRadius: t.radius.pill,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: '#FF3B5C',
          opacity: disabled ? 0.45 : 1,
        }}>
        {busy ? <ActivityIndicator color="#fff" /> : <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: '#fff' }}>{confirmLabel}</Text>}
      </PressableScale>
    </View>
  );
}

/** In-app confirmation dialog (same look on every platform): a title, a message, optional fields, Cancel and a red action. */
export function ConfirmModal({
  visible,
  title,
  message,
  confirmLabel,
  onConfirm,
  onCancel,
  busy,
  confirmDisabled,
  children,
}: {
  visible: boolean;
  title: string;
  message?: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  busy?: boolean;
  confirmDisabled?: boolean;
  children?: ReactNode;
}) {
  return (
    <Dialog visible={visible} onClose={onCancel}>
      <DialogTitle title={title} message={message} />
      {children}
      <DialogActions confirmLabel={confirmLabel} onConfirm={onConfirm} onCancel={onCancel} busy={busy} disabled={confirmDisabled} />
    </Dialog>
  );
}

/** Asked from anywhere (no state to wire up): the one ConfirmHost in the root layout shows it. */
type Ask = { title: string; message?: string; action: string; resolve: (ok: boolean) => void };
let asking: Ask | null = null;
const askListeners = new Set<() => void>();
const setAsking = (next: Ask | null) => {
  asking = next;
  askListeners.forEach((l) => l());
};

export function askConfirm(title: string, message: string | undefined, action: string): Promise<boolean> {
  asking?.resolve(false);
  return new Promise((resolve) => setAsking({ title, message, action, resolve }));
}

export function ConfirmHost() {
  const ask = useSyncExternalStore(
    (l) => {
      askListeners.add(l);
      return () => askListeners.delete(l);
    },
    () => asking,
  );
  const answer = (ok: boolean) => {
    ask?.resolve(ok);
    setAsking(null);
  };
  return (
    <ConfirmModal
      visible={!!ask}
      title={ask?.title ?? ''}
      message={ask?.message}
      confirmLabel={ask?.action ?? ''}
      onConfirm={() => answer(true)}
      onCancel={() => answer(false)}
    />
  );
}

/** The dialog's card: grows in from slightly smaller while the backdrop fades (the Modal remounts it on every open). */
function ModalCard({ children, style }: { children: ReactNode; style: ViewStyle }) {
  const reduced = useReducedMotion();
  const p = useSharedValue(reduced ? 1 : 0);
  useEffect(() => {
    p.value = withTiming(1, { duration: 200, easing: EASE });
  }, [p]);
  const shown = useAnimatedStyle(() => ({ opacity: p.value, transform: [{ scale: 0.94 + 0.06 * p.value }] }));
  return (
    <AnimatedPressable onPress={() => {}} style={[style, shown]}>
      {children}
    </AnimatedPressable>
  );
}
