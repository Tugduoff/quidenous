import { Image } from 'expo-image';
import { RotateCcw, X } from 'lucide-react-native';
import type { ReactNode } from 'react';
import { ActivityIndicator, type ImageStyle, Platform, Pressable, StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import { mediaUrl } from '@/lib/api/client';
import type { Message, MessageMedia } from '@/lib/api/chat';
import { discardPending, type Pending, retryPending } from '@/lib/chat-outbox';
import { dateFormat, useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { frameSize } from '@/lib/utils/chat';

export const MEDIA_RADIUS = 14;

/** Web: no drag-and-drop ghost of the picture, no text selection while zooming. */
export const NO_DRAG = Platform.OS === 'web' ? ({ userSelect: 'none', WebkitUserDrag: 'none' } as unknown as ImageStyle) : null;

/** A photo or GIF at chat size, rounded; `children` float over it (the time, a spinner). */
function MediaFrame({ uri, width, height, label, dim, children }: { uri: string; width: number; height: number; label?: string; dim?: boolean; children?: ReactNode }) {
  const t = useTheme();
  const { width: screen } = useWindowDimensions();
  const size = frameSize(width, height, screen);
  return (
    <View style={[size, { borderRadius: MEDIA_RADIUS, overflow: 'hidden', backgroundColor: t.card2 }]}>
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
function MediaTime({ at }: { at: string }) {
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
