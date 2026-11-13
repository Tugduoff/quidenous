import { Lock } from 'lucide-react-native';
import { useEffect, useRef, useState } from 'react';
import { AppState, StyleSheet, View } from 'react-native';

import { Button, Logo } from '@/components/ui';
import { useT } from '@/lib/i18n';
import { unlock, useLock } from '@/lib/lock';
import { useTheme } from '@/lib/theme';

const AWAY_MS = 5 * 60_000;

/** When the app lock is on: covers everything at a cold start and after 5 min away, until Face ID / fingerprint / passcode. */
export function AppLock({ active }: { active: boolean }) {
  const t = useTheme();
  const tr = useT();
  const { enabled } = useLock();
  const on = enabled && active;
  // Rendered once the saved choice is known: a cold start with the lock on starts locked.
  const [locked, setLocked] = useState(on);
  const leftAt = useRef<number | null>(null);
  const asking = useRef(false);

  const ask = async () => {
    if (asking.current) return;
    asking.current = true;
    try {
      if (await unlock()) setLocked(false);
    } finally {
      asking.current = false;
    }
  };

  useEffect(() => {
    if (!on) return setLocked(false);
    const sub = AppState.addEventListener('change', (state) => {
      // Only 'background' counts: the Face ID sheet itself makes the app 'inactive' on iPhone.
      if (state === 'background') leftAt.current = Date.now();
      if (state === 'active' && leftAt.current != null) {
        if (Date.now() - leftAt.current > AWAY_MS) setLocked(true);
        leftAt.current = null;
      }
    });
    return () => sub.remove();
  }, [on]);

  useEffect(() => {
    if (locked && AppState.currentState === 'active') ask();
  }, [locked]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!locked || !on) return null;
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
