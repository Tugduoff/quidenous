import { type Href, router } from 'expo-router';
import { useEffect, useEffectEvent } from 'react';

export function useRedirect(href: Href | null) {
  useEffect(() => {
    if (href != null) router.replace(href);
  }, [href]);
}

export function useGoBackWhen(when: boolean, fallback: Href) {
  const leave = useEffectEvent(() => {
    if (router.canGoBack()) router.back();
    else router.replace(fallback);
  });
  useEffect(() => {
    if (when) leave();
  }, [when]);
}
