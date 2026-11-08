import type { ReactNode } from 'react';
import { ScrollView, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { useTheme } from '@/lib/theme';

export function Screen({
  children,
  footer,
  tabs = true,
}: {
  children: ReactNode;
  footer?: ReactNode;
  /** Inside the tab bar, which already handles the bottom inset. */
  tabs?: boolean;
}) {
  const t = useTheme();
  return (
    <SafeAreaView edges={tabs ? ['top'] : ['top', 'bottom']} style={{ flex: 1, backgroundColor: t.bg }}>
      <ScrollView contentContainerStyle={{ padding: 20, gap: 16, flexGrow: 1 }}>{children}</ScrollView>
      {footer && <View style={{ paddingHorizontal: 20, paddingBottom: 12 }}>{footer}</View>}
    </SafeAreaView>
  );
}
