import { Check } from 'lucide-react-native';
import { useState } from 'react';
import { Linking, Pressable, Text, View } from 'react-native';
import { API_URL } from '@/lib/api/client';
import { tickTerms, useTermsGate } from '@/lib/hooks/use-terms-gate';
import { useLang, useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import { TextLink } from '@/components/ui/text';

/** `/terms` or `/privacy` in the app's language. */
function openLegal(page: 'terms' | 'privacy', lang: 'fr' | 'en') {
  Linking.openURL(`${API_URL}${lang === 'en' ? '/en' : ''}/${page}`).catch(() => {});
}

/** "J'ai 16 ans ou plus et j'accepte les conditions et la confidentialité", with a box and the two links. */
export function TermsCheck({ value, onChange }: { value: boolean; onChange: (v: boolean) => void }) {
  const t = useTheme();
  const tr = useT();
  const { lang } = useLang();
  const link = { fontFamily: t.font.semibold, color: t.txt, textDecorationLine: 'underline' } as const;
  const toggle = () => {
    tickTerms(!value);
    onChange(!value);
  };
  return (
    <Pressable
      accessibilityRole="checkbox"
      accessibilityState={{ checked: value }}
      accessibilityLabel={tr('legal.accept', { terms: tr('legal.termsLink'), privacy: tr('legal.privacyLink') })}
      onPress={toggle}
      hitSlop={6}
      style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 4 }}>
      <View
        style={{
          width: 22,
          height: 22,
          borderRadius: 7,
          borderWidth: 1.5,
          alignItems: 'center',
          justifyContent: 'center',
          borderColor: value ? t.acc : t.mut,
          backgroundColor: value ? t.acc : 'transparent',
        }}>
        {value && <Check size={15} color={t.accTxt} strokeWidth={3} />}
      </View>
      <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 14, lineHeight: 20, color: t.mut }}>
        {tr('legal.acceptStart')}
        <Text accessibilityRole="link" onPress={() => openLegal('terms', lang)} style={link}>
          {tr('legal.termsLink')}
        </Text>
        {tr('legal.acceptAnd')}
        <Text accessibilityRole="link" onPress={() => openLegal('privacy', lang)} style={link}>
          {tr('legal.privacyLink')}
        </Text>
      </Text>
    </Pressable>
  );
}

/**
 * Signed in with an account that hasn't accepted the current terms (older accounts, Google / Discord
 * sign-ups, or terms that changed): a short sheet asks once. An older server sends no `terms`: nothing.
 */
export function TermsGate() {
  const tr = useT();
  const { signOut } = useSession();
  const { accept, asking } = useTermsGate();
  const [on, setOn] = useState(false);
  return (
    <Dialog visible={asking} onClose={() => {}}>
      <DialogTitle title={tr('legal.gateTitle')} />
      <TermsCheck value={on} onChange={setOn} />
      <View style={{ gap: 4, marginTop: 4 }}>
        <Button onPress={() => accept.mutate()} loading={accept.isPending} disabled={!on}>
          {tr('common.continue')}
        </Button>
        <TextLink onPress={signOut}>{tr('profile.signOut')}</TextLink>
      </View>
    </Dialog>
  );
}
