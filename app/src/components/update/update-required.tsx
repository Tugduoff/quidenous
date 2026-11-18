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

/** Screenshots only (`?maj=required`, in a dev or EXPO_PUBLIC_UPDATE_PREVIEW=1 web build). */
const PREVIEW =
  (__DEV__ || process.env.EXPO_PUBLIC_UPDATE_PREVIEW === '1') && Platform.OS === 'web' && typeof location !== 'undefined'
    ? new URLSearchParams(location.search).get('maj') === 'required'
    : false;

// Once the server said this version is too old, it stays so until the app restarts on a newer one.
let required = PREVIEW;
const listeners = new Set<() => void>();
setOnUpdateRequired(() => {
  if (required) return;
  required = true;
  listeners.forEach((l) => l());
});

/** Gets the newer version: over the air when there is one, else the APK updater, Google Play, or a reload on the web. */
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
    } catch {
      // Offline or no compatible update: a new build it is.
    }
  }
  if (SIDELOAD) {
    await checkForUpdate(true);
    return;
  }
  await Linking.openURL(`market://details?id=${PLAY}`).catch(() => Linking.openURL(`https://play.google.com/store/apps/details?id=${PLAY}`));
}

/** The server no longer serves this version (`426 update_required`): nothing else to do but update. Mounted once, in the root layout. */
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
      {/* Pavé knows: there's a newer version. */}
      <Mascot who="pave" mood="think" size={136} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 22, color: t.txt, textAlign: 'center' }}>{tr('update.requiredTitle')}</Text>
      <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut, textAlign: 'center' }}>{tr('update.requiredBody')}</Text>
      <Button onPress={go} loading={busy} style={{ alignSelf: 'stretch', marginTop: 8 }}>
        {tr('update.requiredAction')}
      </Button>
    </View>
  );
}
