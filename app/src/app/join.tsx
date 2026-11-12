import { router, useLocalSearchParams } from 'expo-router';
import { Mail, Ticket } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';

import { ErrorText, SoftButton } from '@/components/auth-ui';
import { BrandIcon } from '@/components/brand-icons';
import { LegacyJoin } from '@/components/legacy-join';
import { Screen } from '@/components/screen';
import { Button, Logo } from '@/components/ui';
import { authError, isLinked, pendingJoinCode, PROVIDER_NAMES, setPendingCode, useFeatures, type Provider } from '@/lib/auth';
import { oauth, useFinishSignIn } from '@/lib/sign-in';
import { useTheme } from '@/lib/theme';

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
  if (!features.auth) return <LegacyJoin />;
  return <Welcome providers={features.providers} />;
}

function Welcome({ providers }: { providers: Provider[] }) {
  const t = useTheme();
  const finish = useFinishSignIn();
  const [busy, setBusy] = useState<Provider | null>(null);
  const [error, setError] = useState<string | null>(null);
  const invite = pendingJoinCode();

  const withProvider = async (p: Provider) => {
    setBusy(p);
    setError(null);
    try {
      const r = await oauth(p);
      if (r && !isLinked(r)) await finish(r);
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
              {`Continuer avec ${PROVIDER_NAMES[p]}`}
            </SoftButton>
          ))}
          <Button onPress={() => router.push('/auth/email')} disabled={!!busy}>
            <Mail size={20} color={t.accTxt} />
            <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>Continuer avec un e-mail</Text>
          </Button>
        </>
      }>
      <View style={{ marginTop: 48, gap: 14 }}>
        <Logo size={44} />
        <Text style={{ fontFamily: t.font.regular, fontSize: 16, lineHeight: 22, color: t.mut }}>
          Une question. Tout le monde vote.{'\n'}Personne n&apos;est à l&apos;abri.
        </Text>
      </View>
      {invite && (
        <View
          style={{
            alignSelf: 'flex-start',
            flexDirection: 'row',
            alignItems: 'center',
            gap: 8,
            paddingHorizontal: 14,
            paddingVertical: 8,
            borderRadius: t.radius.pill,
            backgroundColor: t.accSoft,
          }}>
          <Ticket size={16} color={t.accInk} />
          <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 14, letterSpacing: 1, color: t.accInk }}>
            {invite}
          </Text>
        </View>
      )}
    </Screen>
  );
}
