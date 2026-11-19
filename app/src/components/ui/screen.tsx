import { CloudOff } from 'lucide-react-native';
import { type ComponentRef, type ReactElement, type ReactNode, type RefObject, useImperativeHandle, useRef } from 'react';
import {
  KeyboardAvoidingView,
  type NativeScrollEvent,
  Platform,
  type RefreshControlProps,
  ScrollView,
  type ScrollViewProps,
  Text,
  View,
  type ViewStyle,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import { useT } from '@/lib/i18n';
import { useUnreachable } from '@/lib/api/live';
import { useTheme } from '@/lib/theme';

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
  scrollEnabled = true,
  refreshControl,
}: {
  children: ReactNode;
  footer?: ReactNode;
  tabs?: boolean;
  onEndReached?: () => void;
  scroller?: RefObject<ComponentRef<typeof ScrollView> | null>;
  onScroll?: (e: NativeScrollEvent) => void;
  onContentSizeChange?: ScrollViewProps['onContentSizeChange'];
  overlay?: ReactNode;
  scrollEnabled?: boolean;
  refreshControl?: ReactElement<RefreshControlProps>;
}) {
  const t = useTheme();
  const inner = useRef<ComponentRef<typeof ScrollView>>(null);
  useImperativeHandle<ComponentRef<typeof ScrollView> | null, ComponentRef<typeof ScrollView> | null>(scroller, () => inner.current);
  return (
    <SafeAreaView
      edges={tabs || Platform.OS === 'web' ? ['top'] : ['top', 'bottom']}
      style={[{ flex: 1, backgroundColor: t.bg }, !tabs && Platform.OS === 'web' && WEB_BOTTOM]}>
      <Unreachable />
      <KeyboardAvoidingView style={{ flex: 1 }} behavior="padding">
        <ScrollView
          ref={inner}
          scrollEnabled={scrollEnabled}
          refreshControl={refreshControl}
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

function Unreachable() {
  const t = useTheme();
  const tr = useT();
  if (!useUnreachable()) return null;
  return (
    <View
      accessibilityRole="alert"
      style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 8, paddingVertical: 6, backgroundColor: t.card }}>
      <CloudOff size={14} color={t.mut} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('common.unreachable')}</Text>
    </View>
  );
}

const nearEnd = ({ layoutMeasurement, contentOffset, contentSize }: NativeScrollEvent) =>
  layoutMeasurement.height + contentOffset.y >= contentSize.height - layoutMeasurement.height;
