import * as Linking from 'expo-linking';
import { router } from 'expo-router';
import * as WebBrowser from 'expo-web-browser';
import { Platform } from 'react-native';

import { ApiError } from './api';
import { auth, flow, isChallenge, type AuthResult, type Provider, type RedeemResult } from './auth';
import { useSession } from './session';

const web = Platform.OS === 'web';
const KEY = 'qdn.oauth';

/** Where the server sends the browser back: this web app's `/oauth`, or the app's `quidenous://oauth`. */
const appRedirect = () => (web ? `${window.location.origin}/oauth` : Linking.createURL('oauth'));

/** The redirect back carries a one-time `code` (or an `error`); with the key from `start` it makes the result. */
export async function redeem(code: string | undefined, error: string | undefined, key: string): Promise<RedeemResult | null> {
  if (error === 'access_denied' || error === 'cancelled') return null;
  if (error || !code) throw new ApiError(400, error || 'bad_code');
  return auth.redeem(code, key);
}

/**
 * Sign in with (or `link`) Google / Discord. The phone opens the provider in an auth session and gets the result
 * (null if cancelled); the web leaves for the provider in the same tab and finishes on `/oauth`.
 */
export async function oauth(provider: Provider, link = false): Promise<RedeemResult | null> {
  const redirect = appRedirect();
  const { url, key } = await auth.oauthStart(provider, redirect, link);
  if (web) {
    sessionStorage.setItem(KEY, key);
    window.location.assign(url);
    return new Promise(() => {}); // the page is leaving
  }
  const res = await WebBrowser.openAuthSessionAsync(url, redirect);
  if (res.type !== 'success') return null;
  const { queryParams } = Linking.parse(res.url);
  const param = (k: string) => (typeof queryParams?.[k] === 'string' ? (queryParams[k] as string) : undefined);
  return redeem(param('code'), param('error'), key);
}

/** Web, back on `/oauth`: the key kept before leaving (once). */
export function takeWebOAuthKey(): string | null {
  if (!web) return null;
  const key = sessionStorage.getItem(KEY);
  sessionStorage.removeItem(KEY);
  return key;
}

/** A sign-in answer: the session (the screens then switch by themselves), or the 2FA step. */
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
