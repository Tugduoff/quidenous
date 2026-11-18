import { Lock } from 'lucide-react-native';
import { StyleSheet, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useLock } from '@/lib/device/lock';
import { useAppLock } from '@/lib/hooks/use-app-lock';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Logo } from '@/components/ui/text';

/** When the app lock is on: covers everything at a cold start and after 5 min away, until Face ID / fingerprint / passcode. */
export function AppLock({ active }: { active: boolean }) {
  const t = useTheme();
  const tr = useT();
  const { enabled } = useLock();
  const { locked, ask } = useAppLock(enabled && active);

  if (!locked) return null;
  return (
    <View style={[StyleSheet.absoluteFill, { backgroundColor: t.bg, alignItems: 'center', justifyContent: 'center', gap: 28, padding: 32 }]}>
      <Logo size={34} />
      <View style={{ width: 72, height: 72, borderRadius: 36, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <Lock size={30} color={t.accInk} />
      </View>
      <Button onPress={ask} style={{ alignSelf: 'stretch', maxWidth: 360, width: '100%' }}>
        {tr('lock.unlock')}
      </Button>
    </View>
  );
}
