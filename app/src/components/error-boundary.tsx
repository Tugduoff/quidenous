import type { ErrorBoundaryProps } from 'expo-router';
import { useEffect } from 'react';
import { Text, View } from 'react-native';

import { Button } from '@/components/ui';
import { reportClientError } from '@/lib/crash-report';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

/** Any screen that throws while rendering lands here (expo-router's ErrorBoundary): the error goes to the server log. */
export function ErrorBoundary({ error, retry }: ErrorBoundaryProps) {
  const t = useTheme();
  const tr = useT();
  useEffect(() => {
    void reportClientError(error);
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
