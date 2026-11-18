import { RotateCw } from 'lucide-react-native';
import { ActivityIndicator, Text, View, type ViewStyle } from 'react-native';
import { Mascot } from '@/components/mascot/mascot';
import { ApiError } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';

type Query = { data?: unknown; isLoading: boolean; isError: boolean; error: unknown; isRefetching?: boolean; refetch: () => unknown };

export function QueryState({ query, style }: { query: Query; style?: ViewStyle }) {
  const t = useTheme();
  const tr = useT();
  if (query.data !== undefined) return null;
  if (query.isLoading) return <ActivityIndicator color={t.mut} style={[{ paddingVertical: 24 }, style]} />;
  if (!query.isError) return null;
  const offline = !(query.error instanceof ApiError);
  return (
    <View accessibilityRole="alert" style={[{ alignItems: 'center', gap: 10, paddingVertical: 16 }, style]}>
      <Mascot who="paillette" mood="sad" size={104} />
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
