import * as ImagePicker from 'expo-image-picker';
import { Camera, Image as ImageIcon, ImagePlus, type LucideIcon, Sticker } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Platform, Pressable, Text, View } from 'react-native';
import { GifPicker } from '@/components/chat/gif-picker';
import { PhotoDialog } from '@/components/chat/photo-dialog';
import { type Anchor, Dialog } from '@/components/ui/dialog';
import { PressableScale } from '@/components/ui/pressable-scale';
import { useFeatures } from '@/lib/api/features';
import type { Member } from '@/lib/api/group';
import { sendGif, sendImage } from '@/lib/chat-outbox';
import { type ImageDraft, prepareImage } from '@/lib/device/media';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { showToast } from '@/lib/toast';
import { outgoing } from '@/lib/utils/mentions';

const IOS_MENU_MODAL_CLOSE_MS = 350;
const iosMenuModalClosed = () => new Promise((resolve) => setTimeout(resolve, IOS_MENU_MODAL_CLOSE_MS));

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

export function AttachButton({
  roundId,
  draft,
  picked,
  others,
  onUseDraft,
  onSent,
}: {
  roundId: number;
  draft: string;
  picked: number[];
  others: Member[];
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
      if (Platform.OS === 'ios') await iosMenuModalClosed();
      if (from === 'camera' && !(await ImagePicker.requestCameraPermissionsAsync()).granted) return showToast(tr('profile.photo.cameraDenied'));
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
