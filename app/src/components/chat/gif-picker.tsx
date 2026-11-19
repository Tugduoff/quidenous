import { Image } from 'expo-image';
import { Search, X } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, type NativeScrollEvent, Pressable, ScrollView, StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import { NO_DRAG } from '@/components/chat/media';
import { Dialog } from '@/components/ui/dialog';
import { Field } from '@/components/ui/field';
import { PressableScale } from '@/components/ui/pressable-scale';
import { type Gif, useGifs } from '@/lib/api/chat';
import { mediaUrl } from '@/lib/api/client';
import { authError } from '@/lib/api/errors';
import { useDebouncedValue } from '@/lib/hooks/use-debounced-value';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { ratioOf, twoColumns } from '@/lib/utils/chat';

export function GifPicker({ visible, onClose, onPick }: { visible: boolean; onClose: () => void; onPick: (g: Gif) => void }) {
  const t = useTheme();
  const tr = useT();
  const { height } = useWindowDimensions();
  const [text, setText] = useState('');
  const q = useDebouncedValue(text, 350).trim();
  const [width, setWidth] = useState(0);
  const gifs = useGifs(q, visible);
  const items = gifs.data?.pages.flatMap((p) => p?.items ?? []) ?? [];
  const GAP = 6;
  const colW = width ? (width - GAP) / 2 : 0;
  const cols = twoColumns(items, colW, GAP);
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
