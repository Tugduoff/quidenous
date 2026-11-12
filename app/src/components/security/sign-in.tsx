import { useQueryClient } from '@tanstack/react-query';
import { BadgeCheck, KeyRound, Mail } from 'lucide-react-native';
import { useRef, useState } from 'react';
import { View } from 'react-native';

import {
  CODE_LENGTH,
  CodeField,
  ErrorText,
  Field,
  type InputRef,
  PasswordField,
  Pill,
  Row,
  Section,
  TextLink,
  useCooldown,
} from '@/components/auth-ui';
import { Toggle } from '@/components/ui';
import { ApiError } from '@/lib/api';
import { accountKey, auth, authError, looksLikeEmail, PASSWORD_MIN, type Account } from '@/lib/auth';
import { useTheme } from '@/lib/theme';

/** Email (+ its code) and password; a legacy account adds both here. */
export function SignInSection({ account }: { account: Account }) {
  const qc = useQueryClient();
  const refresh = () => qc.invalidateQueries({ queryKey: accountKey });
  // The code field opens by itself when the server just emailed one.
  const [entering, setEntering] = useState(false);
  return (
    <Section title="Connexion">
      {account.email ? (
        <>
          <EmailRow account={account} entering={entering} setEntering={setEntering} refresh={refresh} />
          <PasswordRow account={account} refresh={refresh} />
        </>
      ) : (
        <Credentials
          onDone={(sent) => {
            setEntering(sent);
            refresh();
          }}
        />
      )}
    </Section>
  );
}

function Credentials({ onDone }: { onDone: (codeSent: boolean) => void }) {
  const t = useTheme();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const passwordRef = useRef<InputRef>(null);
  const ok = looksLikeEmail(email) && password.length >= PASSWORD_MIN;
  const save = async () => {
    if (!ok || busy) return;
    setBusy(true);
    setError(null);
    try {
      const r = await auth.credentials(email.trim().toLowerCase(), password);
      onDone(!!r?.email_sent);
    } catch (e) {
      setError(authError(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <Row icon={<Mail size={20} color={t.accInk} />} title="Ajoute un e-mail" sub="Pour retrouver ton compte partout" />
      <Field
        inset
        placeholder="toi@exemple.fr"
        accessibilityLabel="E-mail"
        value={email}
        onChangeText={setEmail}
        keyboardType="email-address"
        inputMode="email"
        autoCapitalize="none"
        autoCorrect={false}
        autoComplete="email"
        textContentType="emailAddress"
        returnKeyType="next"
        onSubmitEditing={() => passwordRef.current?.focus()}
      />
      <PasswordField
        inset
        fresh
        ref={passwordRef}
        placeholder={`Mot de passe (${PASSWORD_MIN} min.)`}
        value={password}
        onChangeText={setPassword}
        maxLength={128}
        returnKeyType="go"
        onSubmitEditing={save}
      />
      <ErrorText>{error}</ErrorText>
      <View style={{ alignItems: 'flex-end' }}>
        <Pill tone="acc" onPress={save} loading={busy} disabled={!ok}>
          Enregistrer
        </Pill>
      </View>
    </>
  );
}

function EmailRow({
  account,
  entering,
  setEntering,
  refresh,
}: {
  account: Account;
  entering: boolean;
  setEntering: (v: boolean) => void;
  refresh: () => void;
}) {
  const t = useTheme();
  const [code, setCode] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [wait, startWait] = useCooldown();

  const send = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await auth.sendVerify();
      if (!r.email_sent) setError("Envoi d'e-mails indisponible");
      else {
        setEntering(true);
        startWait();
      }
    } catch (e) {
      if (e instanceof ApiError && e.code === 'already_verified') refresh();
      else setError(authError(e));
    } finally {
      setBusy(false);
    }
  };
  const verify = async (value = code) => {
    if (busy || value.length !== CODE_LENGTH) return;
    setBusy(true);
    setError(null);
    try {
      await auth.verify(value);
      setEntering(false);
      refresh();
    } catch (e) {
      setError(authError(e));
      setCode('');
    } finally {
      setBusy(false);
    }
  };

  const verified = account.email_verified;
  return (
    <>
      <Row
        icon={<Mail size={20} color={t.mut} />}
        title={account.email!}
        sub={verified ? 'Vérifié' : 'Non vérifié'}
        right={
          verified ? (
            <BadgeCheck size={20} color={t.accInk} />
          ) : (
            !entering && (
              <Pill tone="acc" onPress={send} loading={busy}>
                Vérifier
              </Pill>
            )
          )
        }
      />
      {!verified && entering && (
        <>
          <CodeField inset value={code} onChange={setCode} onDone={verify} />
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' }}>
            <TextLink onPress={send} disabled={wait > 0 || busy} style={{ alignSelf: 'auto', paddingHorizontal: 0 }}>
              {wait > 0 ? `Renvoyer (${wait} s)` : 'Renvoyer'}
            </TextLink>
            <Pill tone="acc" onPress={() => verify()} loading={busy} disabled={code.length !== CODE_LENGTH}>
              Valider
            </Pill>
          </View>
        </>
      )}
      <ErrorText>{error}</ErrorText>
    </>
  );
}

function PasswordRow({ account, refresh }: { account: Account; refresh: () => void }) {
  const t = useTheme();
  const has = account.has_password;
  const [open, setOpen] = useState(false);
  const [current, setCurrent] = useState('');
  const [password, setPassword] = useState('');
  const [revoke, setRevoke] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const nextRef = useRef<InputRef>(null);
  const ok = (!has || !!current) && password.length >= PASSWORD_MIN;

  const close = () => {
    setOpen(false);
    setCurrent('');
    setPassword('');
    setError(null);
  };
  const save = async () => {
    if (!ok || busy) return;
    setBusy(true);
    setError(null);
    try {
      const r = await auth.password({ ...(has && { current }), password, revoke_others: revoke });
      close();
      setDone(r.revoked ? `Enregistré · ${r.revoked} appareil${r.revoked > 1 ? 's' : ''} déconnecté${r.revoked > 1 ? 's' : ''}` : 'Enregistré');
      refresh();
    } catch (e) {
      setError(e instanceof ApiError && e.code === 'bad_credentials' ? 'Mot de passe actuel incorrect' : authError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Row
        icon={<KeyRound size={20} color={t.mut} />}
        title="Mot de passe"
        sub={done ?? (has ? '••••••••' : 'Aucun')}
        right={
          !open && (
            <Pill
              onPress={() => {
                setDone(null);
                setOpen(true);
              }}>
              {has ? 'Changer' : 'Définir'}
            </Pill>
          )
        }
      />
      {open && (
        <>
          {has && (
            <PasswordField
              inset
              placeholder="Mot de passe actuel"
              value={current}
              onChangeText={setCurrent}
              autoFocus
              returnKeyType="next"
              onSubmitEditing={() => nextRef.current?.focus()}
            />
          )}
          <PasswordField
            inset
            fresh
            ref={nextRef}
            placeholder={`Nouveau mot de passe (${PASSWORD_MIN} min.)`}
            value={password}
            onChangeText={setPassword}
            autoFocus={!has}
            maxLength={128}
            returnKeyType="go"
            onSubmitEditing={save}
          />
          <Toggle label="Déconnecter les autres appareils" value={revoke} onChange={setRevoke} />
          <ErrorText>{error}</ErrorText>
          <View style={{ flexDirection: 'row', justifyContent: 'flex-end', gap: 8 }}>
            <Pill onPress={close}>Annuler</Pill>
            <Pill tone="acc" onPress={save} loading={busy} disabled={!ok}>
              Enregistrer
            </Pill>
          </View>
        </>
      )}
    </>
  );
}
