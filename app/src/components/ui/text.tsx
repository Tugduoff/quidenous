import { type ReactNode, useState } from 'react';
import { useTheme } from '@/lib/theme';
import { Text, View, type TextStyle, type ViewStyle, Pressable, PixelRatio } from 'react-native';
import { Trophy, Cable, MessageSquareQuote } from 'lucide-react-native';
import { tidy } from '@/lib/utils/text';
import { LogoHat } from '@/components/season/logo-hat';
import { useSeason } from '@/components/season/season-scope';

export function Display({ children, size = 34 }: { children: ReactNode; size?: number }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.display, fontSize: size, lineHeight: size * 1.08, letterSpacing: -1, color: t.txt }}>{children}</Text>;
}

export function CodeText({ code, size = 34, center }: { code: string; size?: number; center?: boolean }) {
  const t = useTheme();
  const [width, setWidth] = useState(0);
  const spacing = code.length > 10 ? 1.5 : 4;
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
  const winter = useSeason() === 'winter';
  const wordmark = (
    <Text style={{ fontFamily: t.font.display, fontSize: size, letterSpacing: -0.5, color: t.txt }}>
      qui de nous<Text style={{ color: t.accInk }}>?</Text>
    </Text>
  );
  if (!winter) return wordmark;
  return (
    <View style={{ flexDirection: 'row' }}>
      <View>
        {wordmark}
        <LogoHat size={size * PixelRatio.getFontScale()} mark={t.accInk} />
      </View>
    </View>
  );
}

export function QuestionText({
  text,
  rank,
  choose,
  said,
  style,
  containerStyle,
}: {
  text: string;
  rank: boolean;
  choose?: boolean;
  said?: boolean;
  style: TextStyle & { fontSize: number; lineHeight: number };
  containerStyle?: ViewStyle;
}) {
  const t = useTheme();
  const size = Math.round(style.fontSize * 0.95);
  return (
    <View style={[{ flexDirection: 'row', alignItems: 'flex-start', gap: 6, flexShrink: 1 }, containerStyle]}>
      {rank && <Trophy size={size} color={t.accInk} style={{ flexShrink: 0, marginTop: (style.lineHeight - size) / 2 }} />}
      {choose && <Cable size={size} color={t.accInk} style={{ flexShrink: 0, marginTop: (style.lineHeight - size) / 2 }} />}
      {said && <MessageSquareQuote size={size} color={t.accInk} style={{ flexShrink: 0, marginTop: (style.lineHeight - size) / 2 }} />}
      <Text style={[{ flex: 1 }, style]}>{tidy(text)}</Text>
    </View>
  );
}

export function TextLink({ children, onPress, disabled, style }: { children: string; onPress: () => void; disabled?: boolean; style?: ViewStyle }) {
  const t = useTheme();
  return (
    <Pressable accessibilityRole="button" onPress={onPress} disabled={disabled} hitSlop={8} style={[{ alignSelf: 'center', padding: 8 }, style]}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: disabled ? t.mut : t.accInk }}>{children}</Text>
    </Pressable>
  );
}

export function ErrorText({ children }: { children: string | null }) {
  const t = useTheme();
  if (!children) return null;
  return <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tagInk, marginLeft: 4 }}>{children}</Text>;
}
