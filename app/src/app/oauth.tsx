import { useLocalSearchParams } from 'expo-router';
import { ActivityIndicator, View } from 'react-native';
import { AuthScreen } from '@/components/auth/auth-screen';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { goHome, useOAuthReturn } from '@/lib/hooks/use-oauth-return';

export default function OAuthReturn() {
  const t = useTheme();
  const tr = useT();
  const { code, error } = useLocalSearchParams<{ code?: string; error?: string }>();
  const failed = useOAuthReturn(code, error);

  if (failed)
    return <AuthScreen back={false} title={tr('auth.oauthFailed')} sub={failed} footer={<Button onPress={goHome}>{tr('common.back')}</Button>} />;
  return (
    <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center', backgroundColor: t.bg }}>
      <ActivityIndicator color={t.mut} />
    </View>
  );
}
