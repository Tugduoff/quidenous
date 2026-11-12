import { router } from 'expo-router';
import { ChevronRight, ShieldAlert, X } from 'lucide-react-native';
import { useSyncExternalStore } from 'react';
import { Pressable, Text, View } from 'react-native';

import { useAccount } from '@/lib/auth';
import { storage } from '@/lib/storage';
import { useTheme } from '@/lib/theme';

const KEY = 'qdn.secureDismissed';
let dismissed: boolean | null = null; // null until read
const listeners = new Set<() => void>();
const set = (v: boolean) => {
  dismissed = v;
  listeners.forEach((l) => l());
};
storage
  .get(KEY)
  .then((v) => set(v === '1'))
  .catch(() => set(false));

/** Home screen, legacy account (token only): a nudge to add an email or link Google / Discord. Dismissible for good. */
export function SecureCard() {
  const t = useTheme();
  const hidden = useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => dismissed,
  );
  const { data: account } = useAccount(hidden === false);
  if (hidden !== false || !account || account.secured) return null;
  return (
    <View style={[{ flexDirection: 'row', alignItems: 'center', borderRadius: t.radius.card, backgroundColor: t.accSoft }]}>
      <Pressable
        accessibilityRole="button"
        onPress={() => router.push('/security')}
        style={{ flex: 1, flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 14, paddingLeft: 16 }}>
        <ShieldAlert size={20} color={t.accInk} />
        <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Sécurise ton compte</Text>
        <ChevronRight size={18} color={t.accInk} />
      </Pressable>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Masquer"
        onPress={() => {
          set(true);
          storage.set(KEY, '1').catch(() => {});
        }}
        hitSlop={8}
        style={{ width: 44, height: 44, alignItems: 'center', justifyContent: 'center' }}>
        <X size={18} color={t.mut} />
      </Pressable>
    </View>
  );
}
