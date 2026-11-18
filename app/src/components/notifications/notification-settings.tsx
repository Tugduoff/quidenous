import { useQueryClient } from '@tanstack/react-query';
import { Bell, BellOff } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Linking, Platform } from 'react-native';
import { useT } from '@/lib/i18n';
import { notificationKeys, type NotifPrefs, useNotifPrefs, useSaveNotifPrefs } from '@/lib/api/notifications';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { ErrorText } from '@/components/ui/text';
import { Toggle } from '@/components/ui/toggle';
import { useFeatures } from '@/lib/api/features';
import { useDevicePush } from '@/lib/hooks/use-device-push';
import { useLastNonNull } from '@/lib/hooks/use-last-non-null';
import { Dim, NotifLine } from '@/components/notifications/notif-line';
import { NotifKindsSection } from '@/components/notifications/notif-kinds-section';
import { QuietHoursCard } from '@/components/notifications/quiet-hours-card';
import { QuietHourDialog } from '@/components/notifications/quiet-hour-dialog';

export function NotificationSettings() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const features = useFeatures();
  const { data: prefs, isError } = useNotifPrefs(features.notificationSettings);
  const save = useSaveNotifPrefs();
  const device = useDevicePush();
  const [enabling, setEnabling] = useState(false);
  const [enableFailed, setEnableFailed] = useState(false);
  const [picking, setPicking] = useState<'start' | 'end' | null>(null);
  // The closing dialog keeps showing what it was picking while it fades out.
  const which = useLastNonNull(picking) ?? 'start';

  // From the cache, not this render: two quick taps both land.
  const set = (patch: Partial<NotifPrefs>) => {
    const current = qc.getQueryData<NotifPrefs>(notificationKeys.prefs);
    if (current) save.mutate({ ...current, ...patch });
  };

  if (!prefs) {
    if (isError || (features.loaded && !features.notificationSettings)) return <ErrorText>{tr('notifications.unavailable')}</ErrorText>;
    return <ActivityIndicator color={t.mut} style={{ marginTop: 24 }} />;
  }
  const state = device.state;
  const needsDevice = prefs.enabled && state === 'off';
  const blocked = state === 'denied';
  const master = prefs.enabled && state !== 'off' && !blocked;
  const sub = blocked
    ? Platform.OS === 'web'
      ? tr('notifications.blockedBrowser')
      : tr('notifications.blockedSettings')
    : needsDevice
      ? tr('notifications.enableHere')
      : state === 'unsupported'
        ? tr('notifications.unsupported')
        : undefined;

  const setMaster = (on: boolean) => {
    setEnableFailed(false);
    if (!on) return set({ enabled: false });
    if (!prefs.enabled) set({ enabled: true });
    // The phone won't ask again: only its settings can.
    if (blocked && Platform.OS !== 'web') return Linking.openSettings();
    if (state !== 'off' && !blocked) return;
    // Straight from the tap: a browser only asks for permission on a user gesture.
    setEnabling(true);
    device
      .enable()
      .catch(() => setEnableFailed(true))
      .finally(() => setEnabling(false));
  };

  const quietHours = prefs.quiet_start != null && prefs.quiet_end != null ? { start: prefs.quiet_start, end: prefs.quiet_end } : null;
  const pickedValue = which === 'start' ? prefs.quiet_start : prefs.quiet_end;
  const otherValue = which === 'start' ? prefs.quiet_end : prefs.quiet_start;

  return (
    <>
      <Card style={{ padding: 16 }}>
        <NotifLine
          Icon={master ? Bell : BellOff}
          on={master}
          title={tr('notifications.title')}
          sub={sub}
          warn={needsDevice || blocked}
          right={
            enabling ? (
              <ActivityIndicator color={t.mut} />
            ) : (
              <Toggle label="" a11y={tr('notifications.title')} value={master} onChange={setMaster} />
            )
          }
        />
      </Card>
      <ErrorText>{enableFailed ? tr('notifications.enableFailed') : save.isError ? tr('notifications.notSaved') : null}</ErrorText>

      <Dim off={!prefs.enabled || blocked}>
        <NotifKindsSection off={prefs.off} streaks={features.streaks} onOff={(off) => set({ off })} />
      </Dim>

      <Dim off={!prefs.enabled || blocked}>
        <QuietHoursCard quietHours={quietHours} onSet={set} onPick={setPicking} />
      </Dim>

      <QuietHourDialog
        visible={picking != null}
        which={which}
        pickedValue={pickedValue}
        otherValue={otherValue}
        onPick={(m) => set(which === 'end' ? { quiet_end: m } : { quiet_start: m })}
        onClose={() => setPicking(null)}
      />
    </>
  );
}
