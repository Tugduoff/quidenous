import { useQueryClient } from '@tanstack/react-query';
import { Copy, LifeBuoy, Share, ShieldCheck, ShieldOff } from 'lucide-react-native';
import { useState } from 'react';
import { Linking, Platform, Text, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';

import { CODE_LENGTH, CodeField, copyText, ErrorText, Field, Pill, Row, Section } from '@/components/auth-ui';
import { PressableScale, Toggle } from '@/components/ui';
import { accountKey, auth, authError, type Account } from '@/lib/auth';
import { useTheme } from '@/lib/theme';

const MONO = Platform.select({ ios: 'Menlo', android: 'monospace', default: 'ui-monospace, Menlo, monospace' });
const native = Platform.OS !== 'web';
const copyLabel = native ? 'Partager' : 'Copier';

type Mode = null | 'setup' | 'regen' | 'disable' | 'codes';

/** TOTP: set up (QR + secret, a code to turn it on, then 10 recovery codes shown once), turn off, new recovery codes. */
export function TwoFactorSection({ account }: { account: Account }) {
  const t = useTheme();
  const qc = useQueryClient();
  const refresh = () => qc.invalidateQueries({ queryKey: accountKey });
  const [mode, setMode] = useState<Mode>(null);
  const [setup, setSetup] = useState<{ secret: string; otpauth_url: string } | null>(null);
  const [codes, setCodes] = useState<string[]>([]);
  const [code, setCode] = useState('');
  const [revoke, setRevoke] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const go = (next: Mode) => {
    setMode(next);
    setCode('');
    setError(null);
  };
  const run = async (f: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(authError(e));
      setCode('');
    } finally {
      setBusy(false);
    }
  };
  const showCodes = (list: string[]) => {
    setCodes(list);
    go('codes');
    refresh();
  };

  const start = () =>
    run(async () => {
      setSetup(await auth.setup2fa());
      go('setup');
    });
  const enable = (value = code) => value.length === CODE_LENGTH && run(async () => showCodes((await auth.enable2fa(value, revoke)).recovery_codes));
  const regen = (value = code) => value.length === CODE_LENGTH && run(async () => showCodes((await auth.recovery(value)).recovery_codes));
  const disable = () =>
    code.trim() &&
    run(async () => {
      await auth.disable2fa(code.trim());
      go(null);
      refresh();
    });

  if (mode === 'codes')
    return (
      <Section title="Double authentification">
        <RecoveryCodes codes={codes} onDone={() => go(null)} />
      </Section>
    );

  if (!account.totp)
    return (
      <Section title="Double authentification">
        {mode === 'setup' && setup ? (
          <>
            <View style={{ alignSelf: 'center', padding: 12, borderRadius: 16, backgroundColor: '#fff' }}>
              <QRCode value={setup.otpauth_url} size={native ? 150 : 176} color="#121216" backgroundColor="#fff" />
            </View>
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
              <Text selectable style={{ flex: 1, fontFamily: MONO, fontSize: 14, letterSpacing: 1, color: t.txt }}>
                {setup.secret.match(/.{1,4}/g)?.join(' ')}
              </Text>
              <PressableScale
                accessibilityRole="button"
                accessibilityLabel={`${copyLabel} la clé`}
                onPress={() => copyText(setup.secret)}
                hitSlop={4}
                style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
                {native ? <Share size={16} color={t.txt} /> : <Copy size={16} color={t.txt} />}
              </PressableScale>
            </View>
            {native && <Pill onPress={() => Linking.openURL(setup.otpauth_url).catch(() => {})}>Ouvrir l&apos;app d&apos;authentification</Pill>}
            <CodeField inset autoFocus={false} value={code} onChange={setCode} onDone={enable} />
            <Toggle label="Déconnecter les autres appareils" value={revoke} onChange={setRevoke} />
            <ErrorText>{error}</ErrorText>
            <View style={{ flexDirection: 'row', justifyContent: 'flex-end', gap: 8 }}>
              <Pill onPress={() => go(null)}>Annuler</Pill>
              <Pill tone="acc" onPress={() => enable()} loading={busy} disabled={code.length !== CODE_LENGTH}>
                Activer
              </Pill>
            </View>
          </>
        ) : (
          <>
            <Row
              icon={<ShieldOff size={20} color={t.mut} />}
              title="Désactivée"
              right={
                <Pill tone="acc" onPress={start} loading={busy}>
                  Activer
                </Pill>
              }
            />
            <ErrorText>{error}</ErrorText>
          </>
        )}
      </Section>
    );

  const left = account.recovery_codes_left;
  return (
    <Section title="Double authentification">
      <Row
        icon={<ShieldCheck size={20} color={t.accInk} />}
        title="Activée"
        right={
          mode == null && (
            <Pill tone="danger" onPress={() => go('disable')}>
              Désactiver
            </Pill>
          )
        }
      />
      <Row
        icon={<LifeBuoy size={20} color={t.mut} />}
        title="Codes de secours"
        sub={`${left} restant${left > 1 ? 's' : ''}`}
        right={mode == null && <Pill onPress={() => go('regen')}>Renouveler</Pill>}
      />
      {mode === 'regen' && <CodeField inset value={code} onChange={setCode} onDone={regen} />}
      {mode === 'disable' && (
        <Field
          inset
          autoFocus
          placeholder="Code de l'app ou de secours"
          value={code}
          onChangeText={setCode}
          autoCapitalize="none"
          autoCorrect={false}
          autoComplete="one-time-code"
          returnKeyType="go"
          onSubmitEditing={disable}
        />
      )}
      <ErrorText>{error}</ErrorText>
      {mode != null && (
        <View style={{ flexDirection: 'row', justifyContent: 'flex-end', gap: 8 }}>
          <Pill onPress={() => go(null)}>Annuler</Pill>
          {mode === 'regen' ? (
            <Pill tone="acc" onPress={() => regen()} loading={busy} disabled={code.length !== CODE_LENGTH}>
              Renouveler
            </Pill>
          ) : (
            <Pill tone="danger" onPress={disable} loading={busy} disabled={!code.trim()}>
              Désactiver
            </Pill>
          )}
        </View>
      )}
    </Section>
  );
}

/** The 10 one-time codes, shown once. */
function RecoveryCodes({ codes, onDone }: { codes: string[]; onDone: () => void }) {
  const t = useTheme();
  const [copied, setCopied] = useState(false);
  return (
    <>
      <Row icon={<LifeBuoy size={20} color={t.accInk} />} title="Tes codes de secours" sub="Affichés une seule fois" />
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', rowGap: 8, padding: 14, borderRadius: 14, backgroundColor: t.card2 }}>
        {codes.map((c) => (
          <Text key={c} selectable style={{ width: '50%', fontFamily: MONO, fontSize: 15, color: t.txt, textAlign: 'center' }}>
            {c}
          </Text>
        ))}
      </View>
      <View style={{ flexDirection: 'row', justifyContent: 'flex-end', gap: 8 }}>
        <Pill onPress={() => copyText(codes.join('\n')).then(setCopied)}>{copied && !native ? 'Copié' : copyLabel}</Pill>
        <Pill tone="acc" onPress={onDone}>
          C&apos;est noté
        </Pill>
      </View>
    </>
  );
}
