import * as Updates from 'expo-updates';
import { useState, useSyncExternalStore } from 'react';
import { Linking, Platform, StyleSheet, Text, View } from 'react-native';
import { Mascot } from '@/components/mascot/mascot';
import { setOnUpdateRequired } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { OTA } from '@/lib/device/ota';
import { useTheme } from '@/lib/theme';
import { checkForUpdate, SIDELOAD } from '@/lib/device/update';
import { Button } from '@/components/ui/button';

const PLAY = 'com.tugduoff.quidenous';

const PREVIEW =
  (__DEV__ || process.env.EXPO_PUBLIC_UPDATE_PREVIEW === '1') && Platform.OS === 'web' && typeof location !== 'undefined'
    ? new URLSearchParams(location.search).get('maj') === 'required'
    : false;

let required = PREVIEW;
const listeners = new Set<() => void>();
setOnUpdateRequired(() => {
  if (required) return;
  required = true;
  listeners.forEach((l) => l());
});

async function update() {
  if (Platform.OS === 'web') {
    location.reload();
    return;
  }
  if (OTA) {
    try {
      if ((await Updates.checkForUpdateAsync()).isAvailable) {
        await Updates.fetchUpdateAsync();
        await Updates.reloadAsync();
        return;
      }
    } catch {}
  }
  if (SIDELOAD) {
    await checkForUpdate(true);
    return;
  }
  await Linking.openURL(`market://details?id=${PLAY}`).catch(() => Linking.openURL(`https://play.google.com/store/apps/details?id=${PLAY}`));
}

export function UpdateRequired() {
  const t = useTheme();
  const tr = useT();
  const on = useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => required,
  );
  const [busy, setBusy] = useState(false);
  if (!on) return null;
  const go = () => {
    setBusy(true);
    update().finally(() => setBusy(false));
  };
  return (
    <View style={[StyleSheet.absoluteFill, { backgroundColor: t.bg, alignItems: 'center', justifyContent: 'center', padding: 32, gap: 16 }]}>
      <Mascot who="pave" mood="think" size={136} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 22, color: t.txt, textAlign: 'center' }}>{tr('update.requiredTitle')}</Text>
      <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut, textAlign: 'center' }}>{tr('update.requiredBody')}</Text>
      <Button onPress={go} loading={busy} style={{ alignSelf: 'stretch', marginTop: 8 }}>
        {tr('update.requiredAction')}
      </Button>
    </View>
  );
}
