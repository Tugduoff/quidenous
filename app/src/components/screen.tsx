import type { ReactNode } from 'react';
import { KeyboardAvoidingView, type NativeScrollEvent, Platform, ScrollView, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { useTheme } from '@/lib/theme';

export function Screen({
  children,
  footer,
  tabs = true,
  onEndReached,
}: {
  children: ReactNode;
  footer?: ReactNode;
  /** Inside the tab bar, which already handles the bottom inset. */
  tabs?: boolean;
  /** Called when the bottom of the content comes within a screen of view, to load more. */
  onEndReached?: () => void;
}) {
  const t = useTheme();
  return (
    <SafeAreaView edges={tabs ? ['top'] : ['top', 'bottom']} style={{ flex: 1, backgroundColor: t.bg }}>
      {/* iOS: lift the footer (message box, buttons) above the keyboard. Android resizes the window itself. */}
      <KeyboardAvoidingView style={{ flex: 1 }} behavior={Platform.OS === 'ios' ? 'padding' : undefined}>
        <ScrollView
          contentContainerStyle={{ padding: 20, gap: 16, flexGrow: 1 }}
          scrollEventThrottle={200}
          onScroll={onEndReached && (({ nativeEvent }) => nearEnd(nativeEvent) && onEndReached())}>
          {children}
        </ScrollView>
        {footer && <View style={{ paddingHorizontal: 20, paddingBottom: 12, gap: 10 }}>{footer}</View>}
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
}

const nearEnd = ({ layoutMeasurement, contentOffset, contentSize }: NativeScrollEvent) =>
  layoutMeasurement.height + contentOffset.y >= contentSize.height - layoutMeasurement.height;
