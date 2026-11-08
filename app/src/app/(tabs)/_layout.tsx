import { Tabs } from 'expo-router';
import { History, MessageCircleQuestion, Vote } from 'lucide-react-native';
import type { ComponentType } from 'react';
import { View } from 'react-native';

import { useTheme } from '@/lib/theme';

type Icon = ComponentType<{ size?: number; color?: string }>;

function TabIcon({ Icon, focused }: { Icon: Icon; focused: boolean }) {
  const t = useTheme();
  const color = focused ? t.accInk : t.mut;
  return (
    <View style={{ alignItems: 'center', gap: 5, paddingTop: 6 }}>
      <Icon size={26} color={color} />
      <View style={{ width: 5, height: 5, borderRadius: 3, backgroundColor: focused ? color : 'transparent' }} />
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
        tabBarStyle: { backgroundColor: t.bg, borderTopColor: t.line, height: 84 },
      }}>
      <Tabs.Screen name="index" options={tab(Vote, 'Vote')} />
      <Tabs.Screen name="history" options={tab(History, 'Historique')} />
      <Tabs.Screen name="questions" options={tab(MessageCircleQuestion, 'Questions')} />
    </Tabs>
  );
}
