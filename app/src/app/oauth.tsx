import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { ActivityIndicator, Platform, View } from 'react-native';

import { AuthScreen } from '@/components/auth-ui';
import { Button } from '@/components/ui';
import { authError, isChallenge, isLinked } from '@/lib/auth';
import { redeem, takeWebOAuthKey, useFinishSignIn } from '@/lib/sign-in';
import { useTheme } from '@/lib/theme';

const home = () => router.replace('/');

/**
 * Back from Google / Discord. Web: `/oauth?code=…` (or `?error=…`) with the key kept before leaving → the session.
 * Phone: the auth session already has the URL; this route only catches the deep link so it doesn't 404.
 */
export default function OAuthReturn() {
  const t = useTheme();
  const { code, error } = useLocalSearchParams<{ code?: string; error?: string }>();
  const finish = useFinishSignIn();
  const [failed, setFailed] = useState<string | null>(null);
  const started = useRef(false);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    if (Platform.OS !== 'web') return void (router.canGoBack() ? router.back() : home());
    const key = takeWebOAuthKey();
    if (!key) return home();
    (async () => {
      try {
        const r = await redeem(code, error, key);
        if (!r) return home();
        if (isLinked(r)) return router.replace('/security');
        await finish(r, true);
        if (!isChallenge(r)) home();
      } catch (e) {
        setFailed(authError(e));
      }
    })();
  }, [code, error, finish]);

  if (failed) return <AuthScreen back={false} title="Connexion impossible" sub={failed} footer={<Button onPress={home}>Retour</Button>} />;
  return (
    <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center', backgroundColor: t.bg }}>
      <ActivityIndicator color={t.mut} />
    </View>
  );
}
