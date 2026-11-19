import { Text } from 'react-native';
import Animated, { FadeIn } from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { useTheme } from '@/lib/theme';
import { sys } from '@/components/wrapped/constants';

export function WrappedMessage({ who, mood, title, line }: { who: 'roupi' | 'pave'; mood: 'sleepy' | 'think'; title: string; line?: string }) {
  const t = useTheme();
  return (
    <Animated.View entering={FadeIn.duration(240).reduceMotion(sys)} style={{ flex: 1, alignItems: 'center', justifyContent: 'center', gap: 8, padding: 32 }}>
      <Mascot who={who} mood={mood} size={140} />
      <Text accessibilityRole="header" style={{ textAlign: 'center', fontFamily: t.font.display, fontSize: 26, lineHeight: 30, letterSpacing: -0.5, color: t.txt }}>
        {title}
      </Text>
      {!!line && <Text style={{ textAlign: 'center', fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{line}</Text>}
    </Animated.View>
  );
}
