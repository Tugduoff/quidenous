import type { ErrorBoundaryProps } from 'expo-router';
import { useEffect } from 'react';
import { Platform, Text, View } from 'react-native';

import { Button } from '@/components/ui';
import { api } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { appVersion } from '@/lib/update';

/** Where it happened (web only: no router hook here, it may be what broke). */
const route = () => (Platform.OS === 'web' && typeof location !== 'undefined' ? location.pathname : undefined);

/** Any screen that throws while rendering lands here (expo-router's ErrorBoundary): the error goes to the server log. */
export function ErrorBoundary({ error, retry }: ErrorBoundaryProps) {
  const t = useTheme();
  const tr = useT();
  useEffect(() => {
    api('/client-errors', {
      method: 'POST',
      json: {
        message: String(error?.message || error).slice(0, 500),
        stack: error?.stack?.slice(0, 4000),
        platform: Platform.OS,
        version: appVersion(),
        route: route(),
      },
    }).catch(() => {});
  }, [error]);
  return (
    <View style={{ flex: 1, backgroundColor: t.bg, alignItems: 'center', justifyContent: 'center', padding: 32, gap: 24 }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: 20, color: t.txt, textAlign: 'center' }}>{tr('common.crashed')}</Text>
      <Button onPress={retry} style={{ alignSelf: 'stretch' }}>
        {tr('common.retry')}
      </Button>
    </View>
  );
}
