import { Image } from 'expo-image';
import * as ImagePicker from 'expo-image-picker';
import { Camera, Download, Image as ImageIcon, ImagePlus, type LucideIcon, RotateCcw, Search, Share, Sticker, X } from 'lucide-react-native';
import { type ComponentRef, type ReactNode, useRef, useState } from 'react';
import { ActivityIndicator, type ImageStyle, type LayoutChangeEvent, Modal, type NativeScrollEvent, Platform, Pressable, ScrollView, StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import { GestureDetector, GestureHandlerRootView, usePanGesture, usePinchGesture, useSimultaneousGestures, useTapGesture } from 'react-native-gesture-handler';
import Animated, { useAnimatedStyle, useReducedMotion, useSharedValue, withTiming } from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { scheduleOnRN } from 'react-native-worklets';
import { showToast } from '@/lib/toast';
import { mediaUrl } from '@/lib/api/client';
import { discardPending, type Pending, retryPending, sendGif, sendImage } from '@/lib/chat-outbox';
import { dateFormat, useT } from '@/lib/i18n';
import { outgoing } from '@/lib/utils/mentions';
import { EASE } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { type Anchor, Dialog, DialogActions } from '@/components/ui/dialog';
import { Field } from '@/components/ui/field';
import { type Member, useMembers } from '@/lib/api/group';
import { type Message, type MessageMedia, type Gif, useGifs } from '@/lib/api/chat';
import { canShareFiles, type ImageDraft, prepareImage, shareMedia } from '@/lib/device/media';
import { mediaOf } from '@/lib/utils/chat';
import { authError } from '@/lib/api/errors';
import { useFeatures } from '@/lib/api/features';
import { useDebouncedValue } from '@/lib/hooks/use-debounced-value';

/** A photo in the chat: at most this tall, at least this wide (a very tall one is cropped rather than a sliver). */
const MAX_H = 320;
const MIN_W = 120;
const RADIUS = 14;

/** Web: no drag-and-drop ghost of the picture, no text selection while zooming. */
const NO_DRAG = Platform.OS === 'web' ? ({ userSelect: 'none', WebkitUserDrag: 'none' } as unknown as ImageStyle) : null;

const ratioOf = (w: number, h: number) => (w > 0 && h > 0 ? w / h : 1);

/** The size a photo takes in a bubble: about 70 % of the screen wide, 320 px tall at most. */
function useFrameSize(width: number, height: number) {
  const { width: screen } = useWindowDimensions();
  const ratio = ratioOf(width, height);
  let w = Math.min(screen * 0.7, 300);
  let h = w / ratio;
  if (h > MAX_H) {
    h = MAX_H;
    w = Math.max(MIN_W, h * ratio);
  }
  // Never wider than the bubble it sits in (narrower than the screen share on small phones): the height follows.
  return { width: Math.round(w), maxWidth: '100%' as const, aspectRatio: w / h };
}

/** A photo or GIF at chat size, rounded; `children` float over it (the time, a spinner). */
export function MediaFrame({ uri, width, height, label, dim, children }: { uri: string; width: number; height: number; label?: string; dim?: boolean; children?: ReactNode }) {
  const t = useTheme();
  const size = useFrameSize(width, height);
  return (
    <View style={[size, { borderRadius: RADIUS, overflow: 'hidden', backgroundColor: t.card2 }]}>
      <Image
        source={{ uri }}
        accessibilityLabel={label}
        contentFit="cover"
        transition={150}
        style={[StyleSheet.absoluteFill, NO_DRAG, dim && { opacity: 0.55 }]}
      />
      {children}
    </View>
  );
}

/** The time over a photo without a caption: white on a dark pill, bottom right. */
export function MediaTime({ at }: { at: string }) {
  const t = useTheme();
  return (
    <View style={{ position: 'absolute', right: 8, bottom: 8, paddingHorizontal: 6, paddingVertical: 2, borderRadius: 8, backgroundColor: 'rgba(0,0,0,0.45)' }}>
      <Text style={{ fontFamily: t.font.regular, fontSize: 10, color: '#fff' }}>{dateFormat({ hour: '2-digit', minute: '2-digit' }).format(new Date(at))}</Text>
    </View>
  );
}

/** The picture a message shows in its bubble: the small rendition when there is one. */
export function MessageMediaFrame({ m, media, label, children }: { m: Message; media: MessageMedia; label: string; children?: ReactNode }) {
  return (
    <MediaFrame uri={mediaUrl(media.preview_url || media.url)} width={media.width} height={media.height} label={label}>
      {children}
      {!m.caption && <MediaTime at={m.created_at} />}
    </MediaFrame>
  );
}

/** A photo or GIF still on its way, mine, at the end of the chat. Failed: a tap sends it again, the cross drops it. */
export function PendingBubble({ roundId, p }: { roundId: number; p: Pending }) {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  return (
    <View style={{ flexDirection: 'row-reverse' }}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={`${tr(p.kind === 'gif' ? 'chat.media.gifBy' : 'chat.media.photoBy', { name: me?.name ?? '' })}, ${tr(p.failed ? 'chat.media.failed' : 'chat.media.sending')}`}
        disabled={!p.failed}
        onPress={() => retryPending(roundId, p.key)}
        style={{ maxWidth: '78%', padding: 4, borderRadius: 18, borderBottomRightRadius: 6, backgroundColor: me?.color ?? t.card2, gap: 4 }}>
        <MediaFrame uri={p.uri} width={p.width} height={p.height} dim>
          <View style={[StyleSheet.absoluteFill, { alignItems: 'center', justifyContent: 'center' }]}>
            {p.failed ? (
              <View style={{ width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: 'rgba(0,0,0,0.55)' }}>
                <RotateCcw size={20} color="#fff" />
              </View>
            ) : (
              <ActivityIndicator color="#fff" />
            )}
          </View>
          {p.failed && (
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={tr('chat.media.discard')}
              hitSlop={8}
              onPress={() => discardPending(roundId, p.key)}
              style={{ position: 'absolute', top: 8, left: 8, width: 28, height: 28, borderRadius: 14, alignItems: 'center', justifyContent: 'center', backgroundColor: 'rgba(0,0,0,0.55)' }}>
              <X size={16} color="#fff" />
            </Pressable>
          )}
        </MediaFrame>
        {!!p.caption && (
          <Text style={{ paddingHorizontal: 10, paddingBottom: 4, fontFamily: t.font.regular, fontSize: 15, lineHeight: 20, color: '#121216' }}>{p.caption}</Text>
        )}
      </Pressable>
    </View>
  );
}

function Row({ icon: Icon, label, onPress }: { icon: LucideIcon; label: string; onPress: () => void }) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      onPress={onPress}
      style={({ pressed }) => ({
        flexDirection: 'row',
        alignItems: 'center',
        gap: 12,
        height: 48,
        paddingHorizontal: 14,
        borderRadius: 14,
        backgroundColor: t.card2,
        opacity: pressed ? 0.7 : 1,
      })}>
      <Icon size={18} color={t.txt} />
      <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
        {label}
      </Text>
    </Pressable>
  );
}

/**
 * The composer's photo / GIF button, and everything it opens: a small menu by it (gallery, camera, GIF), the photo with
 * a caption before it goes (prefilled with what was typed), the GIF search. Nothing at all on a server without either.
 */
export function AttachButton({
  roundId,
  draft,
  picked,
  others,
  onUseDraft,
  onSent,
}: {
  roundId: number;
  /** What's typed in the message box: becomes the caption. */
  draft: string;
  picked: number[];
  others: Member[];
  /** The typed text went out as a caption: the box empties. */
  onUseDraft: () => void;
  onSent?: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  const { chatImages, chatGifs } = useFeatures();
  const button = useRef<ComponentRef<typeof View>>(null);
  const [menu, setMenu] = useState<Anchor | null | false>(false);
  const [photo, setPhoto] = useState<{ img: ImageDraft | null; caption: string } | null>(null);
  const [gifs, setGifs] = useState(false);
  if (!chatImages && !chatGifs) return null;

  const openMenu = () => {
    if (!button.current) return setMenu(null);
    button.current.measureInWindow((_x, y, _w, height) => setMenu(Number.isFinite(y) && height > 0 ? { y, height, side: 'left' } : null));
  };

  const pick = async (from: 'library' | 'camera') => {
    setMenu(false);
    try {
      // iOS can't show the picker while the menu's modal is still closing. (The web must open it right away: a file dialog
      // needs the tap that asked for it.)
      if (Platform.OS === 'ios') await new Promise((r) => setTimeout(r, 350));
      if (from === 'camera' && !(await ImagePicker.requestCameraPermissionsAsync()).granted) return showToast(tr('profile.photo.cameraDenied'));
      // No `exif`: nothing of it is wanted, and the photo is re-encoded without it anyway.
      const options: ImagePicker.ImagePickerOptions = { mediaTypes: ['images'], quality: 1 };
      const res = await (from === 'camera' ? ImagePicker.launchCameraAsync(options) : ImagePicker.launchImageLibraryAsync(options));
      const asset = !res.canceled && res.assets[0];
      if (!asset) return;
      setPhoto({ img: null, caption: draft.trim() });
      const img = await prepareImage(asset);
      if (img === 'too_large') {
        setPhoto(null);
        return showToast(tr('errors.too_large'));
      }
      setPhoto((p) => p && { ...p, img });
    } catch {
      setPhoto(null);
      showToast(tr('chat.media.unreadable'));
    }
  };

  const sendPhoto = () => {
    if (!photo?.img) return;
    const caption = photo.caption.trim();
    const ping = caption ? outgoing(caption, picked, others) : { mentions: [], everyone: false };
    sendImage(roundId, photo.img, caption, ping, onSent);
    // The caption started as the typed text: sent, the box empties (erased, the text stays in the box).
    if (caption) onUseDraft();
    setPhoto(null);
    onSent?.();
  };

  return (
    <>
      <View ref={button} collapsable={false}>
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr('chat.media.attach')}
          onPress={openMenu}
          hitSlop={4}
          style={{ width: 36, height: 40, alignItems: 'center', justifyContent: 'center' }}>
          <ImagePlus size={21} color={t.mut} />
        </PressableScale>
      </View>
      <Dialog visible={menu !== false} onClose={() => setMenu(false)} anchor={menu || undefined}>
        {chatImages && <Row icon={ImageIcon} label={tr('chat.media.library')} onPress={() => pick('library')} />}
        {chatImages && Platform.OS !== 'web' && <Row icon={Camera} label={tr('chat.media.camera')} onPress={() => pick('camera')} />}
        {chatGifs && (
          <Row
            icon={Sticker}
            label={tr('chat.media.gif')}
            onPress={() => {
              setMenu(false);
              setGifs(true);
            }}
          />
        )}
      </Dialog>
      <PhotoDialog
        photo={photo}
        onCaption={(caption) => setPhoto((p) => p && { ...p, caption })}
        onSend={sendPhoto}
        onClose={() => setPhoto(null)}
      />
      <GifPicker
        visible={gifs}
        onClose={() => setGifs(false)}
        onPick={(g) => {
          setGifs(false);
          sendGif(roundId, g, onSent);
          onSent?.();
        }}
      />
    </>
  );
}

/** The picked photo before it goes, with an optional caption. A spinner while it is being resized. */
function PhotoDialog({
  photo,
  onCaption,
  onSend,
  onClose,
}: {
  photo: { img: ImageDraft | null; caption: string } | null;
  onCaption: (caption: string) => void;
  onSend: () => void;
  onClose: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  const [boxW, setBoxW] = useState(0);
  // The last one stays drawn while the dialog fades out.
  const [shown, setShown] = useState(photo);
  if (photo && photo !== shown) setShown(photo);
  const img = shown?.img;
  const ratio = img ? ratioOf(img.width, img.height) : 4 / 3;
  const h = boxW ? Math.min(300, boxW / ratio) : 0;
  return (
    <Dialog visible={!!photo} onClose={onClose}>
      <View onLayout={(e: LayoutChangeEvent) => setBoxW(e.nativeEvent.layout.width)} style={{ alignItems: 'center' }}>
        {img && h ? (
          <Image
            source={{ uri: img.uri }}
            contentFit="cover"
            style={{ width: Math.min(boxW, h * ratio), height: h, borderRadius: RADIUS, backgroundColor: t.card2 }}
          />
        ) : (
          <View style={{ width: '100%', height: 200, borderRadius: RADIUS, backgroundColor: t.card2, alignItems: 'center', justifyContent: 'center' }}>
            <ActivityIndicator color={t.mut} />
          </View>
        )}
      </View>
      <Field
        inset
        accessibilityLabel={tr('chat.media.captionLabel')}
        placeholder={tr('chat.media.caption')}
        value={shown?.caption ?? ''}
        onChangeText={onCaption}
        maxLength={500}
        returnKeyType="send"
        onSubmitEditing={onSend}
        style={{ fontFamily: t.font.regular, paddingVertical: 12 }}
      />
      <DialogActions tone="acc" confirmLabel={tr('common.send')} onConfirm={onSend} onCancel={onClose} disabled={!img} />
    </Dialog>
  );
}

/** Trending GIFs, or a search (typed, then 350 ms of quiet), two columns, more as it scrolls. A tap sends one. */
function GifPicker({ visible, onClose, onPick }: { visible: boolean; onClose: () => void; onPick: (g: Gif) => void }) {
  const t = useTheme();
  const tr = useT();
  const { height } = useWindowDimensions();
  const [text, setText] = useState('');
  const q = useDebouncedValue(text, 350).trim();
  const [width, setWidth] = useState(0);
  const gifs = useGifs(q, visible);
  const items = gifs.data?.pages.flatMap((p) => p?.items ?? []) ?? [];
  // Two columns, each GIF going under the shorter one.
  const GAP = 6;
  const colW = width ? (width - GAP) / 2 : 0;
  const cols: Gif[][] = [[], []];
  const heights = [0, 0];
  for (const g of items) {
    const c = heights[0] <= heights[1] ? 0 : 1;
    cols[c].push(g);
    heights[c] += colW / ratioOf(g.width, g.height) + GAP;
  }
  const more = (e: NativeScrollEvent) => {
    if (e.contentOffset.y + e.layoutMeasurement.height < e.contentSize.height - 400) return;
    if (gifs.hasNextPage && !gifs.isFetchingNextPage) void gifs.fetchNextPage();
  };
  return (
    <Dialog visible={visible} onClose={onClose}>
      <Field
        inset
        placeholder={tr('chat.media.search')}
        value={text}
        onChangeText={setText}
        returnKeyType="search"
        autoCorrect={false}
        style={{ fontFamily: t.font.regular, paddingVertical: 12 }}
        right={
          text ? (
            <Pressable accessibilityRole="button" accessibilityLabel={tr('common.close')} hitSlop={8} onPress={() => setText('')} style={{ paddingRight: 14 }}>
              <X size={18} color={t.mut} />
            </Pressable>
          ) : (
            <Search size={18} color={t.mut} style={{ marginRight: 14 }} />
          )
        }
      />
      <ScrollView
        onLayout={(e) => setWidth(e.nativeEvent.layout.width)}
        style={{ height: Math.min(460, height * 0.55) }}
        keyboardShouldPersistTaps="handled"
        keyboardDismissMode="on-drag"
        scrollEventThrottle={200}
        onScroll={(e) => more(e.nativeEvent)}>
        {gifs.isError && !items.length ? (
          <Text style={{ textAlign: 'center', marginTop: 40, fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{authError(gifs.error)}</Text>
        ) : gifs.isSuccess && !items.length ? (
          <Text style={{ textAlign: 'center', marginTop: 40, fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{tr('chat.media.noGifs')}</Text>
        ) : !colW || (!items.length && gifs.isFetching) ? (
          <ActivityIndicator color={t.mut} style={{ marginTop: 40 }} />
        ) : (
          <View style={{ flexDirection: 'row', gap: GAP, alignItems: 'flex-start' }}>
            {cols.map((col, i) => (
              <View key={i} style={{ width: colW, gap: GAP }}>
                {col.map((g) => (
                  <PressableScale
                    key={String(g.id)}
                    accessibilityRole="button"
                    accessibilityLabel={g.title || tr('chat.media.gif')}
                    onPress={() => onPick(g)}
                    style={{ width: colW, height: colW / ratioOf(g.width, g.height), borderRadius: 10, overflow: 'hidden', backgroundColor: t.card2 }}>
                    <Image source={{ uri: mediaUrl(g.preview_url) }} contentFit="cover" style={[StyleSheet.absoluteFill, NO_DRAG]} />
                  </PressableScale>
                ))}
              </View>
            ))}
          </View>
        )}
        {gifs.isFetchingNextPage && <ActivityIndicator color={t.mut} style={{ marginVertical: 12 }} />}
      </ScrollView>
    </Dialog>
  );
}

/** Zoom limits in the viewer, and how far a swipe goes before it closes it. */
const MAX_ZOOM = 5;
const DOUBLE_ZOOM = 2.5;
const CLOSE_DRAG = 120;
const CLOSE_SPEED = 900;

/** A photo or GIF full screen: pinch, drag, double tap to zoom, swipe down to close; share / save. */
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
  // Pinch: the scale and offset it started from, and its focal point (from the centre).
  const s0 = useSharedValue(1);
  const x0 = useSharedValue(0);
  const y0 = useSharedValue(0);
  const fx = useSharedValue(0);
  const fy = useSharedValue(0);
  // Drag: where the picture was when it started.
  const px = useSharedValue(0);
  const py = useSharedValue(0);
  // Swipe to close (not zoomed): how far down / up.
  const drag = useSharedValue(0);

  const timing = (v: number) => {
    'worklet';
    return reduced ? v : withTiming(v, { duration: 200, easing: EASE });
  };
  // A zoomed picture never leaves an empty edge on screen.
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
      // The point under the fingers stays under them.
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
      // Zoom in on the tapped point.
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
