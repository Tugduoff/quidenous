import { Image } from 'expo-image';
import { Download, Share, X } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Modal, StyleSheet, Text, useWindowDimensions } from 'react-native';
import { GestureDetector, GestureHandlerRootView, usePanGesture, usePinchGesture, useSimultaneousGestures, useTapGesture } from 'react-native-gesture-handler';
import Animated, { useAnimatedStyle, useReducedMotion, useSharedValue, withTiming } from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { scheduleOnRN } from 'react-native-worklets';
import { NO_DRAG } from '@/components/chat/media';
import { PressableScale } from '@/components/ui/pressable-scale';
import type { Message, MessageMedia } from '@/lib/api/chat';
import { mediaUrl } from '@/lib/api/client';
import { useMembers } from '@/lib/api/group';
import { canShareFiles, shareMedia } from '@/lib/device/media';
import { useT } from '@/lib/i18n';
import { EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { showToast } from '@/lib/toast';
import { mediaOf, ratioOf } from '@/lib/utils/chat';

const MAX_ZOOM = 5;
const DOUBLE_ZOOM = 2.5;
const CLOSE_DRAG = 120;
const CLOSE_SPEED = 900;

export function MediaViewer({ message, onClose }: { message: Message | null; onClose: () => void }) {
  const reduced = useReducedMotion();
  const media = message && mediaOf(message);
  if (!message || !media) return null;
  return (
    <Modal visible transparent animationType={reduced ? 'none' : 'fade'} statusBarTranslucent navigationBarTranslucent onRequestClose={onClose}>
      <Viewer m={message} media={media} onClose={onClose} />
    </Modal>
  );
}

function Viewer({ m, media, onClose }: { m: Message; media: MessageMedia; onClose: () => void }) {
  const t = useTheme();
  const tr = useT();
  const insets = useSafeAreaInsets();
  const reduced = useReducedMotion();
  const { byId } = useMembers();
  const name = byId.get(m.member_id)?.name ?? '';
  const label = tr(m.kind === 'gif' ? 'chat.media.gifBy' : 'chat.media.photoBy', { name });
  const win = useWindowDimensions();
  const [box, setBox] = useState({ w: win.width, h: win.height });
  const [busy, setBusy] = useState(false);
  const W = box.w;
  const H = box.h;
  const ratio = ratioOf(media.width, media.height);
  const dw = Math.min(W, H * ratio);
  const dh = dw / ratio;

  const scale = useSharedValue(1);
  const tx = useSharedValue(0);
  const ty = useSharedValue(0);
  const s0 = useSharedValue(1);
  const x0 = useSharedValue(0);
  const y0 = useSharedValue(0);
  const fx = useSharedValue(0);
  const fy = useSharedValue(0);
  const px = useSharedValue(0);
  const py = useSharedValue(0);
  const drag = useSharedValue(0);

  const timing = (v: number) => {
    'worklet';
    return reduced ? v : withTiming(v, { duration: 200, easing: EASE });
  };
  const clampX = (x: number, s: number) => {
    'worklet';
    const max = Math.max(0, (dw * s - W) / 2);
    return Math.min(max, Math.max(-max, x));
  };
  const clampY = (y: number, s: number) => {
    'worklet';
    const max = Math.max(0, (dh * s - H) / 2);
    return Math.min(max, Math.max(-max, y));
  };

  const pinch = usePinchGesture({
    onActivate: (e) => {
      'worklet';
      s0.value = scale.value;
      x0.value = tx.value;
      y0.value = ty.value;
      fx.value = e.focalX - W / 2;
      fy.value = e.focalY - H / 2;
    },
    onUpdate: (e) => {
      'worklet';
      const s = Math.min(MAX_ZOOM, Math.max(0.8, s0.value * e.scale));
      const r = s / s0.value;
      scale.value = s;
      tx.value = fx.value - (fx.value - x0.value) * r;
      ty.value = fy.value - (fy.value - y0.value) * r;
    },
    onDeactivate: () => {
      'worklet';
      const s = Math.max(1, scale.value);
      scale.value = timing(s);
      tx.value = timing(s === 1 ? 0 : clampX(tx.value, s));
      ty.value = timing(s === 1 ? 0 : clampY(ty.value, s));
    },
  });

  const pan = usePanGesture({
    maxPointers: 1,
    activeOffsetX: [-10, 10],
    activeOffsetY: [-10, 10],
    onActivate: () => {
      'worklet';
      px.value = tx.value;
      py.value = ty.value;
    },
    onUpdate: (e) => {
      'worklet';
      if (scale.value > 1.01) {
        tx.value = clampX(px.value + e.translationX, scale.value);
        ty.value = clampY(py.value + e.translationY, scale.value);
      } else drag.value = e.translationY;
    },
    onDeactivate: (e) => {
      'worklet';
      if (drag.value === 0) return;
      if (Math.abs(drag.value) > CLOSE_DRAG || Math.abs(e.velocityY) > CLOSE_SPEED) {
        if (reduced) scheduleOnRN(onClose);
        else drag.value = withTiming(Math.sign(drag.value || e.velocityY) * H, { duration: 180, easing: EASE }, () => scheduleOnRN(onClose));
      } else drag.value = timing(0);
    },
  });

  const doubleTap = useTapGesture({
    numberOfTaps: 2,
    onActivate: (e) => {
      'worklet';
      if (scale.value > 1.01) {
        scale.value = timing(1);
        tx.value = timing(0);
        ty.value = timing(0);
        return;
      }
      const x = e.x - W / 2;
      const y = e.y - H / 2;
      scale.value = timing(DOUBLE_ZOOM);
      tx.value = timing(clampX(-x * (DOUBLE_ZOOM - 1), DOUBLE_ZOOM));
      ty.value = timing(clampY(-y * (DOUBLE_ZOOM - 1), DOUBLE_ZOOM));
    },
  });

  const gesture = useSimultaneousGestures(pinch, pan, doubleTap);

  const picture = useAnimatedStyle(() => ({
    transform: [{ translateX: tx.value }, { translateY: ty.value + drag.value }, { scale: scale.value }],
  }));
  const backdrop = useAnimatedStyle(() => ({ opacity: 1 - Math.min(0.8, Math.abs(drag.value) / H) }));
  const chrome = useAnimatedStyle(() => ({ opacity: Math.max(0, 1 - Math.abs(drag.value) / 80) }));

  const share = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await shareMedia(m);
    } catch {
      showToast(tr('common.failed'));
    } finally {
      setBusy(false);
    }
  };
  const sharing = canShareFiles();
  const ShareIcon = sharing ? Share : Download;
  const round = { width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: 'rgba(0,0,0,0.45)' } as const;

  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <Animated.View style={[StyleSheet.absoluteFill, { backgroundColor: '#000' }, backdrop]} />
      <GestureDetector gesture={gesture} touchAction="none">
        <Animated.View
          accessibilityViewIsModal
          onLayout={(e) => setBox({ w: e.nativeEvent.layout.width, h: e.nativeEvent.layout.height })}
          style={{ flex: 1, alignItems: 'center', justifyContent: 'center', overflow: 'hidden' }}>
          <Animated.View style={[{ width: dw, height: dh }, picture]}>
            <Image
              source={{ uri: mediaUrl(media.url) }}
              placeholder={media.preview_url ? { uri: mediaUrl(media.preview_url) } : undefined}
              placeholderContentFit="contain"
              contentFit="contain"
              accessibilityLabel={label}
              style={[{ width: '100%', height: '100%' }, NO_DRAG]}
            />
          </Animated.View>
        </Animated.View>
      </GestureDetector>
      <Animated.View
        pointerEvents="box-none"
        style={[{ position: 'absolute', top: insets.top + 8, left: 12, right: 12, flexDirection: 'row', justifyContent: 'space-between' }, chrome]}>
        <PressableScale accessibilityRole="button" accessibilityLabel={tr('common.close')} onPress={onClose} style={round}>
          <X size={22} color="#fff" />
        </PressableScale>
        <PressableScale accessibilityRole="button" accessibilityLabel={tr(sharing ? 'chat.media.share' : 'chat.media.save')} onPress={share} style={round}>
          {busy ? <ActivityIndicator color="#fff" /> : <ShareIcon size={20} color="#fff" />}
        </PressableScale>
      </Animated.View>
      {!!m.caption && (
        <Animated.View
          pointerEvents="none"
          style={[{ position: 'absolute', left: 0, right: 0, bottom: 0, paddingHorizontal: 20, paddingTop: 14, paddingBottom: insets.bottom + 18, backgroundColor: 'rgba(0,0,0,0.5)', gap: 2 }, chrome]}>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 13, color: '#fff', opacity: 0.8 }}>
            {name}
          </Text>
          <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: '#fff' }}>{m.caption}</Text>
        </Animated.View>
      )}
    </GestureHandlerRootView>
  );
}
