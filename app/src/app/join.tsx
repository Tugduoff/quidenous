import { router, useLocalSearchParams } from 'expo-router';
import { Mail } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import { BrandIcon } from '@/components/auth/brand-icons';
import { InvitePreview } from '@/components/rooms/invite-preview';
import { QueryState } from '@/components/ui/query-state';
import { LegacyJoin } from '@/components/rooms/legacy-join';
import { MascotBand } from '@/components/mascot/band';
import { Screen } from '@/components/ui/screen';
import { authError, isLinked, isReauth, pendingJoinCode, PROVIDER_NAMES, setPendingCode, useFeatures, type Provider } from '@/lib/auth';
import { useT } from '@/lib/i18n';
import { oauth, useFinishSignIn } from '@/lib/sign-in';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Logo, ErrorText } from '@/components/ui/text';
import { SoftButton } from '@/components/ui/soft-button';

/** The entry screen. An invite link (`?code=`) is kept for after signing in. An older server keeps the old join screen. */
export default function Entry() {
  const t = useTheme();
  const { code } = useLocalSearchParams<{ code?: string }>();
  useState(() => code && setPendingCode(code));
  const features = useFeatures();
  if (!features.loaded)
    return (
      <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center', backgroundColor: t.bg }}>
        <ActivityIndicator color={t.mut} />
      </View>
    );
  // Offline or server down: a retry, not the old join screen (that is for a server too old to have accounts).
  if (features.unreachable)
    return (
      <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center', backgroundColor: t.bg, padding: 24 }}>
        <QueryState query={features.query} />
      </View>
    );
  if (!features.auth) return <LegacyJoin />;
  return <Welcome providers={features.providers} />;
}

function Welcome({ providers }: { providers: Provider[] }) {
  const t = useTheme();
  const tr = useT();
  const finish = useFinishSignIn();
  const [busy, setBusy] = useState<Provider | null>(null);
  const [error, setError] = useState<string | null>(null);
  const invite = pendingJoinCode();

  const withProvider = async (p: Provider) => {
    setBusy(p);
    setError(null);
    try {
      const r = await oauth(p);
      if (r && !isLinked(r) && !isReauth(r)) await finish(r);
    } catch (e) {
      setError(authError(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Screen
      tabs={false}
      footer={
        <>
          <ErrorText>{error}</ErrorText>
          {providers.map((p) => (
            <SoftButton key={p} icon={<BrandIcon provider={p} />} loading={busy === p} disabled={!!busy} onPress={() => withProvider(p)}>
              {tr('auth.welcome.continueWith', { provider: PROVIDER_NAMES[p] })}
            </SoftButton>
          ))}
          <Button onPress={() => router.push('/auth/email')} disabled={!!busy}>
            <Mail size={20} color={t.accTxt} />
            <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{tr('auth.welcome.continueEmail')}</Text>
          </Button>
        </>
      }>
      <View style={{ marginTop: 48, gap: 14 }}>
        <Logo size={44} />
        <Text style={{ fontFamily: t.font.regular, fontSize: 16, lineHeight: 22, color: t.mut }}>{tr('auth.tagline')}</Text>
      </View>
      {/* "Personne n'est à l'abri": the whole band points at Bulle. */}
      <MascotBand max={invite ? 340 : 460} style={{ marginTop: invite ? 4 : 40 }} />
      {invite && <InvitePreview code={invite} />}
    </Screen>
  );
}
