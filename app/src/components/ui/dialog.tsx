import { type ReactNode, useState } from 'react';
import { useTheme } from '@/lib/theme';
import { useT } from '@/lib/i18n';
import { type ViewStyle, Modal, KeyboardAvoidingView, Platform, Pressable, StyleSheet, View, useWindowDimensions, Text, ActivityIndicator, type LayoutChangeEvent } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { PressableScale } from '@/components/ui/pressable-scale';
import Animated, { useReducedMotion, useSharedValue, withTiming, useAnimatedStyle } from 'react-native-reanimated';
import { EASE } from '@/lib/motion';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';

export type Anchor = { y: number; height: number; side: 'left' | 'right' };

export function Dialog({ visible, onClose, anchor, children }: { visible: boolean; onClose: () => void; anchor?: Anchor; children: ReactNode }) {
  const t = useTheme();
  const tr = useT();
  const card: ViewStyle = { borderRadius: 24, backgroundColor: t.card, borderWidth: 1, borderColor: t.line };
  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : undefined} style={{ flex: 1 }}>
        <Pressable
          accessibilityLabel={tr('common.close')}
          onPress={onClose}
          style={[StyleSheet.absoluteFill, { backgroundColor: anchor ? t.dialogScrim.anchored : t.dialogScrim.centered }]}
        />
        {anchor ? (
          <Anchored anchor={anchor} style={{ ...card, padding: 10, gap: 6 }}>
            {children}
          </Anchored>
        ) : (
          <View pointerEvents="box-none" style={{ flex: 1, alignItems: 'center', justifyContent: 'center', padding: 24 }}>
            <ModalCard style={{ ...card, width: '100%', maxWidth: 380, padding: 22, gap: 12 }}>{children}</ModalCard>
          </View>
        )}
      </KeyboardAvoidingView>
    </Modal>
  );
}

function Anchored({ anchor, style, children }: { anchor: Anchor; style: ViewStyle; children: ReactNode }) {
  const { width, height } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const [cardHeight, setCardHeight] = useState(0);
  const min = insets.top + 12;
  const max = height - insets.bottom - 12 - cardHeight;
  const below = anchor.y + anchor.height + 8;
  const above = anchor.y - 8 - cardHeight;
  const top = below <= max ? below : Math.max(min, Math.min(above, max));
  return (
    <ModalCard
      onLayout={(e) => setCardHeight(e.nativeEvent.layout.height)}
      style={{ ...style, position: 'absolute', top, width: Math.min(340, width - 32), [anchor.side]: 16 }}>
      {children}
    </ModalCard>
  );
}

export function DialogTitle({ title, message }: { title: string; message?: string }) {
  const t = useTheme();
  return (
    <>
      <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{title}</Text>
      {!!message && <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{message}</Text>}
    </>
  );
}

export function DialogActions({
  confirmLabel,
  onConfirm,
  onCancel,
  busy,
  disabled,
  tone = 'danger',
}: {
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  busy?: boolean;
  disabled?: boolean;
  tone?: 'danger' | 'acc';
}) {
  const t = useTheme();
  const tr = useT();
  const off = busy || disabled;
  const ink = tone === 'acc' ? t.accTxt : t.onStrong;
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
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('common.cancel')}</Text>
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
          backgroundColor: tone === 'acc' ? t.acc : t.dangerFill,
          opacity: disabled ? 0.45 : 1,
        }}>
        {busy ? <ActivityIndicator color={ink} /> : <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: ink }}>{confirmLabel}</Text>}
      </PressableScale>
    </View>
  );
}

export function ConfirmModal({
  visible,
  title,
  message,
  confirmLabel,
  onConfirm,
  onCancel,
  busy,
  confirmDisabled,
  tone,
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
  tone?: 'danger' | 'acc';
  children?: ReactNode;
}) {
  return (
    <Dialog visible={visible} onClose={onCancel}>
      <DialogTitle title={title} message={message} />
      {children}
      <DialogActions confirmLabel={confirmLabel} onConfirm={onConfirm} onCancel={onCancel} busy={busy} disabled={confirmDisabled} tone={tone} />
    </Dialog>
  );
}

function ModalCard({ children, style, onLayout }: { children: ReactNode; style: ViewStyle; onLayout?: (e: LayoutChangeEvent) => void }) {
  const reduced = useReducedMotion();
  const p = useSharedValue(reduced ? 1 : 0);
  useAnimateOnChange(p, 1, () => withTiming(1, { duration: 200, easing: EASE }));
  const shown = useAnimatedStyle(() => ({ opacity: p.value, transform: [{ scale: 0.94 + 0.06 * p.value }] }));
  return (
    <Animated.View accessibilityViewIsModal onLayout={onLayout} style={[style, shown]}>
      {children}
    </Animated.View>
  );
}
