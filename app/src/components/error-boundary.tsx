import type { ErrorBoundaryProps } from 'expo-router';
import { useEffect } from 'react';
import { Text, View } from 'react-native';

import { Mascot } from '@/components/mascot';
import { Button } from '@/components/ui';
import type { Pose } from '@/lib/mascot';
import { reportClientError } from '@/lib/crash-report';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

/** Paillette, knocked flat. */
const KNOCKED: Pose = { eyes: 'wide', brows: 'worried', mouth: 'gasp', arms: ['droop', 'droop'], legs: ['sit', 'sit'], prop: 'sweat', tilt: -7 };

/** Any screen that throws while rendering lands here (expo-router's ErrorBoundary): the error goes to the server log. */
export function ErrorBoundary({ error, retry }: ErrorBoundaryProps) {
  const t = useTheme();
  const tr = useT();
  useEffect(() => {
    void reportClientError(error);
  }, [error]);
  return (
    <View style={{ flex: 1, backgroundColor: t.bg, alignItems: 'center', justifyContent: 'center', padding: 32, gap: 24 }}>
      <Mascot who="paillette" mood={KNOCKED} motion="caught" size={140} style={{ marginBottom: -12 }} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 20, color: t.txt, textAlign: 'center' }}>{tr('common.crashed')}</Text>
      <Button onPress={retry} style={{ alignSelf: 'stretch' }}>
        {tr('common.retry')}
      </Button>
    </View>
  );
}
