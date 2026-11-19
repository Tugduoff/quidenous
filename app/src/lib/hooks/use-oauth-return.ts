import { router } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { Platform } from 'react-native';
import { isChallenge, isLinked, isReauth } from '@/lib/api/auth';
import { authError } from '@/lib/api/errors';
import { useT } from '@/lib/i18n';
import { redeem, takeWebOAuthKey, useFinishSignIn } from '@/lib/sign-in';
import { showToast } from '@/lib/toast';

export const goHome = () => router.replace('/');

export function useOAuthReturn(code: string | undefined, error: string | undefined) {
  const tr = useT();
  const finish = useFinishSignIn();
  const [failed, setFailed] = useState<string | null>(null);
  const started = useRef(false);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    if (Platform.OS !== 'web') return void (router.canGoBack() ? router.back() : goHome());
    const key = takeWebOAuthKey();
    if (!key) return goHome();
    (async () => {
      try {
        const r = await redeem(code, error, key);
        if (!r) return goHome();
        if (isLinked(r)) return router.replace('/security');
        if (isReauth(r)) {
          showToast(tr('security.reauth.done'));
          return router.replace('/security');
        }
        await finish(r, true);
        if (!isChallenge(r)) goHome();
      } catch (e) {
        setFailed(authError(e));
      }
    })();
  }, [code, error, finish, tr]);

  return failed;
}
