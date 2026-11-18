import { type Href, router } from 'expo-router';
import { useEffect } from 'react';

export function useRedirect(href: Href | null) {
  useEffect(() => {
    if (href != null) router.replace(href);
  }, [href]);
}
