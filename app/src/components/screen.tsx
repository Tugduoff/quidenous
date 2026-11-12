import { type ComponentRef, type ReactNode, type RefObject, useEffect, useRef } from 'react';
import { KeyboardAvoidingView, type NativeScrollEvent, Platform, ScrollView, type ScrollViewProps, View, type ViewStyle } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { useTheme } from '@/lib/theme';

// Home screen web app on iPhone: the safe-area inset is only known to CSS (same as the tab bar).
const WEB_BOTTOM = { paddingBottom: 'env(safe-area-inset-bottom)' } as unknown as ViewStyle;

export function Screen({
  children,
  footer,
  tabs = true,
  onEndReached,
  scroller,
  onScroll,
  onContentSizeChange,
  overlay,
}: {
  children: ReactNode;
  footer?: ReactNode;
  /** Inside the tab bar, which already handles the bottom inset. */
  tabs?: boolean;
  /** Called when the bottom of the content comes within a screen of view, to load more. */
  onEndReached?: () => void;
  /** The page's ScrollView, to scroll it from outside. (Not named "…Ref": the React Compiler would take it for a stable ref.) */
  scroller?: RefObject<ComponentRef<typeof ScrollView> | null>;
  onScroll?: (e: NativeScrollEvent) => void;
  onContentSizeChange?: ScrollViewProps['onContentSizeChange'];
  /** Floats over the top of the content (a pinned summary), outside the scroll. */
  overlay?: ReactNode;
}) {
  const t = useTheme();
  // The web's ScrollView keeps the ref it was mounted with: one of our own, handed out after each render.
  const inner = useRef<ComponentRef<typeof ScrollView>>(null);
  useEffect(() => {
    if (scroller) scroller.current = inner.current;
  });
  return (
    <SafeAreaView
      edges={tabs || Platform.OS === 'web' ? ['top'] : ['top', 'bottom']}
      style={[{ flex: 1, backgroundColor: t.bg }, !tabs && Platform.OS === 'web' && WEB_BOTTOM]}>
      {/* Lift the footer (message box, buttons) above the keyboard. Android too: the app is edge-to-edge,
          so the system no longer shrinks the window when the keyboard opens. */}
      <KeyboardAvoidingView style={{ flex: 1 }} behavior="padding">
        <ScrollView
          ref={inner}
          contentContainerStyle={{ padding: 20, gap: 16, flexGrow: 1 }}
          scrollEventThrottle={onScroll ? 32 : 200}
          onContentSizeChange={onContentSizeChange}
          onScroll={
            (onEndReached || onScroll) &&
            (({ nativeEvent }) => {
              onScroll?.(nativeEvent);
              if (onEndReached && nearEnd(nativeEvent)) onEndReached();
            })
          }>
          {children}
        </ScrollView>
        {overlay}
        {footer && <View style={{ paddingHorizontal: 20, paddingBottom: 12, gap: 10 }}>{footer}</View>}
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
}

const nearEnd = ({ layoutMeasurement, contentOffset, contentSize }: NativeScrollEvent) =>
  layoutMeasurement.height + contentOffset.y >= contentSize.height - layoutMeasurement.height;
