import { useLocalSearchParams } from 'expo-router';
import { ActivityIndicator, View } from 'react-native';
import { AuthScreen } from '@/components/auth/auth-screen';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { goHome, useOAuthReturn } from '@/lib/hooks/use-oauth-return';

/**
 * Back from Google / Discord. Web: `/oauth?code=…` (or `?error=…`) with the key kept before leaving → the session.
 * Phone: the auth session already has the URL; this route only catches the deep link so it doesn't 404.
 */
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
