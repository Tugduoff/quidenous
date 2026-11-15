import { CloudOff, RotateCw } from 'lucide-react-native';
import { ActivityIndicator, Text, View, type ViewStyle } from 'react-native';

import { PressableScale } from '@/components/ui';
import { ApiError } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

type Query = { data?: unknown; isLoading: boolean; isError: boolean; error: unknown; isRefetching?: boolean; refetch: () => unknown };

/**
 * What a screen shows while its data loads (a spinner) or when it failed (offline or error, and Retry).
 * Nothing once there is data: a failed refresh keeps what's on screen.
 */
export function QueryState({ query, style }: { query: Query; style?: ViewStyle }) {
  const t = useTheme();
  const tr = useT();
  if (query.data !== undefined) return null;
  if (query.isLoading) return <ActivityIndicator color={t.mut} style={[{ paddingVertical: 24 }, style]} />;
  if (!query.isError) return null;
  const offline = !(query.error instanceof ApiError);
  return (
    <View accessibilityRole="alert" style={[{ alignItems: 'center', gap: 12, paddingVertical: 24 }, style]}>
      <CloudOff size={28} color={t.mut} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut, textAlign: 'center' }}>
        {tr(offline ? 'common.offline' : 'common.loadFailed')}
      </Text>
      <PressableScale
        accessibilityRole="button"
        disabled={query.isRefetching}
        onPress={() => query.refetch()}
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          gap: 8,
          height: 40,
          paddingHorizontal: 18,
          borderRadius: t.radius.pill,
          backgroundColor: t.card,
          borderWidth: 1,
          borderColor: t.line,
        }}>
        {query.isRefetching ? <ActivityIndicator size="small" color={t.mut} /> : <RotateCw size={16} color={t.txt} />}
        <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>{tr('common.retry')}</Text>
      </PressableScale>
    </View>
  );
}
