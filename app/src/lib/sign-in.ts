import * as Linking from 'expo-linking';
import { router } from 'expo-router';
import * as WebBrowser from 'expo-web-browser';
import { Platform } from 'react-native';
import { ApiError } from '@/lib/api/client';
import { auth, isChallenge, type AuthResult, type OAuthMode, type Provider, type RedeemResult } from '@/lib/api/auth';
import { useSession } from '@/lib/session';
import { flow } from '@/lib/auth-flow';

const web = Platform.OS === 'web';
const KEY = 'qdn.oauth';

const appRedirect = () => (web ? `${window.location.origin}/oauth` : Linking.createURL('oauth'));

export async function redeem(code: string | undefined, error: string | undefined, key: string): Promise<RedeemResult | null> {
  if (error === 'access_denied' || error === 'cancelled') return null;
  if (error || !code) throw new ApiError(400, error || 'bad_code');
  return auth.redeem(code, key);
}

export async function oauth(provider: Provider, mode: OAuthMode = 'signin'): Promise<RedeemResult | null> {
  const redirect = appRedirect();
  const { url, key } = await auth.oauthStart(provider, redirect, mode);
  if (web) {
    sessionStorage.setItem(KEY, key);
    window.location.assign(url);
    return new Promise((resolve) => {
      const back = (e: PageTransitionEvent) => {
        if (!e.persisted) return;
        window.removeEventListener('pageshow', back);
        sessionStorage.removeItem(KEY);
        resolve(null);
      };
      window.addEventListener('pageshow', back);
    });
  }
  const res = await WebBrowser.openAuthSessionAsync(url, redirect);
  if (res.type !== 'success') return null;
  const { queryParams } = Linking.parse(res.url);
  const param = (k: string) => (typeof queryParams?.[k] === 'string' ? (queryParams[k] as string) : undefined);
  return redeem(param('code'), param('error'), key);
}

export function takeWebOAuthKey(): string | null {
  if (!web) return null;
  const key = sessionStorage.getItem(KEY);
  sessionStorage.removeItem(KEY);
  return key;
}

export function useFinishSignIn() {
  const { signIn } = useSession();
  return async (r: AuthResult, replace = false) => {
    if (isChallenge(r)) {
      flow.challenge = r.challenge;
      if (replace) router.replace('/auth/two-factor');
      else router.push('/auth/two-factor');
      return;
    }
    await signIn(r);
  };
}
