import { Image } from 'expo-image';
import { useState } from 'react';
import { ActivityIndicator, type LayoutChangeEvent, View } from 'react-native';
import { MEDIA_RADIUS } from '@/components/chat/media';
import { Dialog, DialogActions } from '@/components/ui/dialog';
import { Field } from '@/components/ui/field';
import type { ImageDraft } from '@/lib/device/media';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { ratioOf } from '@/lib/utils/chat';

export function PhotoDialog({
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
            style={{ width: Math.min(boxW, h * ratio), height: h, borderRadius: MEDIA_RADIUS, backgroundColor: t.card2 }}
          />
        ) : (
          <View style={{ width: '100%', height: 200, borderRadius: MEDIA_RADIUS, backgroundColor: t.card2, alignItems: 'center', justifyContent: 'center' }}>
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
