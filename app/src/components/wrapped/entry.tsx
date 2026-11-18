import { router } from 'expo-router';
import { CalendarHeart, ChevronRight } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import Svg, { Defs, LinearGradient, Rect, Stop } from 'react-native-svg';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { useWrappedEntry } from '@/lib/api/wrapped';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';

import { INK } from '@/components/wrapped/constants';
import { ACCENTS } from '@/lib/tokens';
const open = (ask: boolean) => router.push(ask ? '/wrapped?preview=1' : '/wrapped');

export function WrappedBanner() {
  const t = useTheme();
  const tr = useT();
  const entry = useWrappedEntry();
  const [box, setBox] = useState({ w: 0, h: 0 });
  if (!entry) return null;
  const title = tr('wrapped.entry');
  const line = tr('wrapped.entryLine', { year: entry.year });
  return (
    <Animated.View entering={motion.drop}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        accessibilityLabel={`${title}${entry.preview ? ` (${tr('wrapped.preview')})` : ''}. ${line}`}
        onPress={() => open(entry.ask)}
        onLayout={(e) => setBox({ w: e.nativeEvent.layout.width, h: e.nativeEvent.layout.height })}
        style={{ flexDirection: 'row', alignItems: 'center', gap: 14, padding: 18, borderRadius: 24, overflow: 'hidden', backgroundColor: ACCENTS.rose.acc }}>
        {box.w > 0 && (
          <Svg width={box.w} height={box.h} style={{ position: 'absolute', top: 0, left: 0 }}>
            <Defs>
              <LinearGradient id="wrapped-banner" x1="0" y1="0" x2="1" y2="1">
                <Stop offset="0" stopColor={ACCENTS.rose.acc} />
                <Stop offset="0.55" stopColor={ACCENTS.orange.acc} />
                <Stop offset="1" stopColor={ACCENTS.lime.acc} />
              </LinearGradient>
            </Defs>
            <Rect x="0" y="0" width={box.w} height={box.h} fill="url(#wrapped-banner)" />
          </Svg>
        )}
        <View style={{ width: 48, height: 48, borderRadius: 24, alignItems: 'center', justifyContent: 'center', backgroundColor: INK }}>
          <CalendarHeart size={24} color={ACCENTS.orange.acc} strokeWidth={2.25} />
        </View>
        <View style={{ flex: 1, gap: 2 }}>
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.display, fontSize: 20, letterSpacing: -0.4, color: INK }}>
              {title}
            </Text>
            {entry.preview && <PreviewTag ink={INK} />}
          </View>
          <Text numberOfLines={2} style={{ fontFamily: t.font.semibold, fontSize: 14, lineHeight: 19, color: INK }}>
            {line}
          </Text>
        </View>
        <ChevronRight size={22} color={INK} />
      </PressableScale>
    </Animated.View>
  );
}

export function WrappedRow() {
  const t = useTheme();
  const tr = useT();
  const entry = useWrappedEntry();
  if (!entry) return null;
  return (
    <Pressable accessibilityRole="button" accessibilityLabel={`${tr('wrapped.entry')}${entry.preview ? ` (${tr('wrapped.preview')})` : ''}`} onPress={() => open(entry.ask)}>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
        <CalendarHeart size={20} color={t.txt} />
        <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
          {tr('wrapped.entry')}
        </Text>
        {entry.preview && <PreviewTag ink={t.mut} />}
        <ChevronRight size={20} color={t.mut} />
      </Card>
    </Pressable>
  );
}

function PreviewTag({ ink }: { ink: string }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ paddingHorizontal: 8, height: 20, borderRadius: 10, justifyContent: 'center', borderWidth: 1.5, borderColor: ink }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: 10, letterSpacing: 0.5, textTransform: 'uppercase', color: ink }}>{tr('wrapped.preview')}</Text>
    </View>
  );
}
