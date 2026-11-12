import { Tabs } from 'expo-router';
import { ChartNoAxesColumn, History, MessageCircleQuestion, Vote } from 'lucide-react-native';
import type { ComponentType } from 'react';
import { useEffect } from 'react';
import { Platform, View, type ViewStyle } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion, useSharedValue, withSpring } from 'react-native-reanimated';

import { Celebration } from '@/components/celebration';
import { useMilestone } from '@/lib/milestones';
import { BOUNCE, SPRING, usePop } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

type Icon = ComponentType<{ size?: number; color?: string }>;

// Home screen web app on iPhone: the safe-area inset is only known to CSS, so pad with env().
const WEB_TAB_BAR = {
  height: 'auto',
  paddingBottom: 'max(env(safe-area-inset-bottom), 8px)',
} as unknown as ViewStyle;

/** The icon pops when its tab gets selected; the dot under it stretches into a short pill. */
function TabIcon({ Icon, focused }: { Icon: Icon; focused: boolean }) {
  const t = useTheme();
  const color = focused ? t.accInk : t.mut;
  const pop = usePop(focused, 1.15, focused);
  const reduced = useReducedMotion();
  const on = useSharedValue(focused ? 1 : 0);
  useEffect(() => {
    on.value = reduced ? (focused ? 1 : 0) : withSpring(focused ? 1 : 0, focused ? BOUNCE : SPRING);
  }, [focused, on, reduced]);
  const pill = useAnimatedStyle(() => ({ width: 4 + 12 * on.value, opacity: on.value }));
  return (
    <View style={{ alignItems: 'center', gap: 4 }}>
      <Animated.View style={pop}>
        <Icon size={24} color={color} />
      </Animated.View>
      <Animated.View style={[{ height: 4, borderRadius: 2, backgroundColor: t.accInk }, pill]} />
    </View>
  );
}

export default function TabsLayout() {
  const t = useTheme();
  const { milestone, dismiss } = useMilestone();
  const reduced = useReducedMotion();
  const tab = (Icon: Icon, label: string) => ({
    tabBarAccessibilityLabel: label,
    tabBarIcon: ({ focused }: { focused: boolean }) => <TabIcon Icon={Icon} focused={focused} />,
  });
  return (
    <View style={{ flex: 1 }}>
      <Tabs
        screenOptions={{
          headerShown: false,
          // Switching tabs: the new screen fades in with a small sideways shift (still under reduced motion).
          animation: reduced ? 'none' : 'shift',
          transitionSpec: { animation: 'timing', config: { duration: 180 } },
          sceneStyle: { backgroundColor: t.bg },
          tabBarShowLabel: false,
          tabBarStyle: [{ backgroundColor: t.bg, borderTopColor: t.line }, Platform.OS === 'web' && WEB_TAB_BAR],
          tabBarItemStyle: { paddingTop: 6 },
        }}>
        <Tabs.Screen name="index" options={tab(Vote, 'Vote')} />
        <Tabs.Screen name="history" options={tab(History, 'Historique')} />
        <Tabs.Screen name="stats" options={tab(ChartNoAxesColumn, 'Stats')} />
        <Tabs.Screen name="questions" options={tab(MessageCircleQuestion, 'Questions')} />
      </Tabs>
      {milestone != null && <Celebration key={milestone} count={milestone} onDone={dismiss} />}
    </View>
  );
}
