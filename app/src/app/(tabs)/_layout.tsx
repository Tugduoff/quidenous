import { Tabs } from 'expo-router';
import { History, MessageCircleQuestion, Vote } from 'lucide-react-native';
import type { ComponentType } from 'react';
import { Platform, View, type ViewStyle } from 'react-native';

import { useTheme } from '@/lib/theme';

type Icon = ComponentType<{ size?: number; color?: string }>;

// Home screen web app on iPhone: the safe-area inset is only known to CSS, so pad with env().
const WEB_TAB_BAR = {
  height: 'auto',
  paddingBottom: 'max(env(safe-area-inset-bottom), 8px)',
} as unknown as ViewStyle;

function TabIcon({ Icon, focused }: { Icon: Icon; focused: boolean }) {
  const t = useTheme();
  const color = focused ? t.accInk : t.mut;
  return (
    <View style={{ alignItems: 'center', gap: 4 }}>
      <Icon size={24} color={color} />
      <View style={{ width: 4, height: 4, borderRadius: 2, backgroundColor: focused ? color : 'transparent' }} />
    </View>
  );
}

export default function TabsLayout() {
  const t = useTheme();
  const tab = (Icon: Icon, label: string) => ({
    tabBarAccessibilityLabel: label,
    tabBarIcon: ({ focused }: { focused: boolean }) => <TabIcon Icon={Icon} focused={focused} />,
  });
  return (
    <Tabs
      screenOptions={{
        headerShown: false,
        tabBarShowLabel: false,
        tabBarStyle: [
          { backgroundColor: t.bg, borderTopColor: t.line },
          Platform.OS === 'web' && WEB_TAB_BAR,
        ],
        tabBarItemStyle: { paddingTop: 6 },
      }}>
      <Tabs.Screen name="index" options={tab(Vote, 'Vote')} />
      <Tabs.Screen name="history" options={tab(History, 'Historique')} />
      <Tabs.Screen name="questions" options={tab(MessageCircleQuestion, 'Questions')} />
    </Tabs>
  );
}
