import { Tabs, useSegments } from 'expo-router';
import { ChartNoAxesColumn, History, MessageCircleQuestion, Sun, Vote } from 'lucide-react-native';
import { type ComponentType } from 'react';
import { Platform, View, type ViewStyle } from 'react-native';
import Animated, { useAnimatedStyle, useReducedMotion, useSharedValue, withSpring } from 'react-native-reanimated';
import { Celebration } from '@/components/celebration';
import { TabSwipe } from '@/components/ui/tab-swipe';
import { OutsideRoom } from '@/components/themes/room-theme-scope';
import { type TKey, useT } from '@/lib/i18n';
import { useTodayTab } from '@/lib/hooks/use-today-tab';
import { BOUNCE, SPRING, usePop } from '@/lib/motion';
import { useOwnTheme, useTheme } from '@/lib/theme';
import { useStreakMilestone } from '@/lib/hooks/use-streak-milestone';
import { useMilestone } from '@/lib/hooks/use-milestone';
import { useAnimateOnChange } from '@/lib/hooks/use-animate-on-change';

type Icon = ComponentType<{ size?: number; color?: string }>;

const TODAY = { name: 'today', Icon: Sun as Icon, label: 'today.tab' as TKey };
const TABS: { name: string; Icon: Icon; label: TKey }[] = [
  { name: 'index', Icon: Vote, label: 'vote.tabs.vote' },
  { name: 'history', Icon: History, label: 'vote.tabs.history' },
  { name: 'stats', Icon: ChartNoAxesColumn, label: 'vote.tabs.stats' },
  { name: 'questions', Icon: MessageCircleQuestion, label: 'vote.tabs.questions' },
];

const WEB_TAB_BAR = {
  height: 'auto',
  paddingBottom: 'max(env(safe-area-inset-bottom), 8px)',
} as unknown as ViewStyle;

function TabIcon({ Icon, focused }: { Icon: Icon; focused: boolean }) {
  const t = useTheme();
  const color = focused ? t.accInk : t.mut;
  const pop = usePop(focused, 1.15, focused);
  const reduced = useReducedMotion();
  const on = useSharedValue(focused ? 1 : 0);
  useAnimateOnChange(on, [focused, reduced], () => (reduced ? (focused ? 1 : 0) : withSpring(focused ? 1 : 0, focused ? BOUNCE : SPRING)));
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
  const room = useTheme();
  const mine = useOwnTheme();
  const t = useSegments().at(-1) === TODAY.name ? mine : room;
  const tr = useT();
  const { milestone, dismiss } = useMilestone();
  const streak = useStreakMilestone();
  const reduced = useReducedMotion();
  const today = useTodayTab();
  const tabs = today ? [TODAY, ...TABS] : TABS;
  const tab = (Icon: Icon, label: string) => ({
    tabBarAccessibilityLabel: label,
    tabBarIcon: ({ focused }: { focused: boolean }) => <TabIcon Icon={Icon} focused={focused} />,
  });
  return (
    <View style={{ flex: 1 }}>
      <Tabs
        layout={({ state, navigation, children }) => (
          <OutsideRoom when={state.routes[state.index]?.name === TODAY.name}>
            <TabSwipe
              onSwipe={(dir) => {
                const to = tabs[tabs.findIndex((s) => s.name === state.routes[state.index]?.name) + dir];
                if (!to || !state.routeNames.includes(to.name)) return;
                navigation.navigate(to.name, state.routes.find((r) => r.name === to.name)?.params);
              }}>
              {children}
            </TabSwipe>
          </OutsideRoom>
        )}
        screenOptions={{
          headerShown: false,
          animation: reduced ? 'none' : 'shift',
          transitionSpec: { animation: 'timing', config: { duration: 180 } },
          sceneStyle: { backgroundColor: t.bg },
          tabBarShowLabel: false,
          tabBarStyle: [{ backgroundColor: t.bg, borderTopColor: t.line }, Platform.OS === 'web' && WEB_TAB_BAR],
          tabBarItemStyle: { paddingTop: 6 },
        }}>
        {!today && <Tabs.Screen name="today" options={{ href: null }} />}
        {tabs.map((s) => (
          <Tabs.Screen key={s.name} name={s.name} options={tab(s.Icon, tr(s.label))} />
        ))}
      </Tabs>
      {milestone != null && <Celebration key={milestone} count={milestone} onDone={dismiss} />}
      {milestone == null && streak.milestone != null && <Celebration key={`s${streak.milestone}`} streak count={streak.milestone} onDone={streak.dismiss} />}
    </View>
  );
}
