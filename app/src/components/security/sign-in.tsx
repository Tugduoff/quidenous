import { useQueryClient } from '@tanstack/react-query';
import { BadgeCheck, KeyRound, Mail } from 'lucide-react-native';
import { useRef, useState } from 'react';
import { View } from 'react-native';
import { withReauth } from '@/components/security/reauth';
import { ApiError } from '@/lib/api/client';
import { accountKey, auth, authError, looksLikeEmail, PASSWORD_MIN, useFeatures, type Account } from '@/lib/api/auth';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Toggle } from '@/components/ui/toggle';
import { CODE_LENGTH, CodeField, Field, type InputRef, PasswordField } from '@/components/ui/field';
import { ErrorText, TextLink } from '@/components/ui/text';
import { Pill } from '@/components/ui/pill';
import { Row, Section } from '@/components/ui/section';
import { useCooldown } from '@/lib/hooks/use-cooldown';

/** Email (+ its code) and password; a legacy account adds both here. */
export function SignInSection({ account }: { account: Account }) {
  const tr = useT();
  const qc = useQueryClient();
  const refresh = () => qc.invalidateQueries({ queryKey: accountKey });
  // The code field opens by itself when the server just emailed one (or tried to: then it says so).
  const [entering, setEntering] = useState(false);
  const [mailFailed, setMailFailed] = useState(false);
  return (
    <Section title={tr('security.signIn.title')}>
      {account.email ? (
        <>
          <EmailRow account={account} entering={entering} setEntering={setEntering} refresh={refresh} mailFailed={mailFailed} />
          <PasswordRow account={account} refresh={refresh} />
        </>
      ) : (
        <Credentials
          account={account}
          onDone={(sent, failed) => {
            setEntering(sent || failed);
            setMailFailed(failed);
            refresh();
          }}
        />
      )}
    </Section>
  );
}

function Credentials({ account, onDone }: { account: Account; onDone: (codeSent: boolean, failed: boolean) => void }) {
  const t = useTheme();
  const tr = useT();
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
      // An account signed in with a provider that gave no e-mail proves it is me first.
      const r = await withReauth(account, () => auth.credentials(email.trim().toLowerCase(), password));
      if (r === null) return;
      onDone(!!r?.email_sent, !!r?.email_failed);
    } catch (e) {
      setError(authError(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <Row icon={<Mail size={20} color={t.accInk} />} title={tr('security.signIn.addEmail')} />
      <Field
        inset
        placeholder={tr('auth.emailPlaceholder')}
        accessibilityLabel={tr('auth.emailLabel')}
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
        placeholder={tr('auth.newPassword', { min: PASSWORD_MIN })}
        value={password}
        onChangeText={setPassword}
        maxLength={128}
        returnKeyType="go"
        onSubmitEditing={save}
      />
      <ErrorText>{error}</ErrorText>
      <View style={{ alignItems: 'flex-end' }}>
        <Pill tone="acc" onPress={save} loading={busy} disabled={!ok}>
          {tr('common.save')}
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
  mailFailed,
}: {
  account: Account;
  entering: boolean;
  setEntering: (v: boolean) => void;
  refresh: () => void;
  mailFailed: boolean;
}) {
  const t = useTheme();
  const tr = useT();
  const features = useFeatures();
  const [code, setCode] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(() => (mailFailed ? tr('errors.email_failed') : null));
  const [wait, startWait] = useCooldown();
  const [changing, setChanging] = useState(false);

  const send = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await auth.sendVerify();
      if (!r.email_sent) setError(tr('errors.email_not_configured'));
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
  // A typo or a new address: a code to the new one first (older servers can't).
  const canChange = features.email && features.accountRecovery;
  if (changing)
    return (
      <ChangeEmail
        account={account}
        onClose={(changed) => {
          setChanging(false);
          if (changed) {
            setEntering(false);
            setError(null);
            refresh();
          }
        }}
      />
    );
  return (
    <>
      <Row
        icon={verified ? <BadgeCheck size={20} color={t.accInk} /> : <Mail size={20} color={t.mut} />}
        title={account.email!}
        sub={verified ? tr('security.signIn.verified') : tr('security.signIn.notVerified')}
        right={
          !entering && (
            <View style={{ flexDirection: 'row', gap: 8 }}>
              {canChange && <Pill onPress={() => setChanging(true)}>{tr('security.signIn.change')}</Pill>}
              {!verified && (
                <Pill tone="acc" onPress={send} loading={busy}>
                  {tr('security.signIn.verify')}
                </Pill>
              )}
            </View>
          )
        }
      />
      {!verified && entering && (
        <>
          <CodeField inset value={code} onChange={setCode} onDone={verify} />
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' }}>
            <TextLink onPress={send} disabled={wait > 0 || busy} style={{ alignSelf: 'auto', paddingHorizontal: 0 }}>
              {wait > 0 ? tr('auth.resendIn', { count: wait }) : tr('auth.resend')}
            </TextLink>
            <Pill tone="acc" onPress={() => verify()} loading={busy} disabled={code.length !== CODE_LENGTH}>
              {tr('auth.validate')}
            </Pill>
          </View>
        </>
      )}
      <ErrorText>{error}</ErrorText>
    </>
  );
}

/** New address → a code sent to it → confirmed: it replaces the old one (which gets a notice). */
function ChangeEmail({ account, onClose }: { account: Account; onClose: (changed: boolean) => void }) {
  const t = useTheme();
  const tr = useT();
  const [email, setEmail] = useState('');
  const [sentTo, setSentTo] = useState<string | null>(null);
  const [code, setCode] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [wait, startWait] = useCooldown();
  const value = email.trim().toLowerCase();
  const ok = looksLikeEmail(value);

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
  const send = () =>
    ok &&
    run(async () => {
      // The address that resets the password: proof that it is me first.
      const r = await withReauth(account, () => auth.changeEmail(value));
      if (!r) return;
      setSentTo(value);
      startWait();
    });
  const confirm = (c = code) =>
    !!sentTo &&
    c.length === CODE_LENGTH &&
    run(async () => {
      await auth.confirmEmail(sentTo, c);
      onClose(true);
    });

  return (
    <>
      <Row icon={<Mail size={20} color={t.accInk} />} title={tr('security.signIn.newEmail')} sub={sentTo ? tr('auth.codeSentTo', { email: sentTo }) : account.email!} />
      {sentTo ? (
        <>
          <CodeField inset value={code} onChange={setCode} onDone={confirm} />
          <TextLink onPress={send} disabled={wait > 0 || busy} style={{ alignSelf: 'flex-start', paddingHorizontal: 0 }}>
            {wait > 0 ? tr('auth.resendIn', { count: wait }) : tr('auth.resend')}
          </TextLink>
        </>
      ) : (
        <Field
          inset
          autoFocus
          placeholder={tr('auth.emailPlaceholder')}
          accessibilityLabel={tr('security.signIn.newEmail')}
          value={email}
          onChangeText={setEmail}
          keyboardType="email-address"
          inputMode="email"
          autoCapitalize="none"
          autoCorrect={false}
          autoComplete="email"
          textContentType="emailAddress"
          returnKeyType="go"
          onSubmitEditing={send}
        />
      )}
      <ErrorText>{error}</ErrorText>
      <View style={{ flexDirection: 'row', justifyContent: 'flex-end', gap: 8 }}>
        <Pill onPress={() => onClose(false)}>{tr('common.cancel')}</Pill>
        {sentTo ? (
          <Pill tone="acc" onPress={() => confirm()} loading={busy} disabled={code.length !== CODE_LENGTH}>
            {tr('auth.validate')}
          </Pill>
        ) : (
          <Pill tone="acc" onPress={send} loading={busy} disabled={!ok}>
            {tr('security.signIn.sendCode')}
          </Pill>
        )}
      </View>
    </>
  );
}

function PasswordRow({ account, refresh }: { account: Account; refresh: () => void }) {
  const t = useTheme();
  const tr = useT();
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
      // A first password (no current one to type) needs proof that it is me first.
      const r = await withReauth(account, () => auth.password({ ...(has && { current }), password, revoke_others: revoke }));
      if (!r) return;
      close();
      setDone(r.revoked ? tr('security.signIn.revoked', { count: r.revoked }) : tr('security.signIn.saved'));
      refresh();
    } catch (e) {
      setError(e instanceof ApiError && e.code === 'bad_credentials' ? tr('security.signIn.badCurrent') : authError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Row
        icon={<KeyRound size={20} color={t.mut} />}
        title={tr('security.signIn.password')}
        sub={done ?? (has ? '••••••••' : tr('security.signIn.none'))}
        right={
          !open && (
            <Pill
              onPress={() => {
                setDone(null);
                setOpen(true);
              }}>
              {has ? tr('security.signIn.change') : tr('security.signIn.set')}
            </Pill>
          )
        }
      />
      {open && (
        <>
          {has && (
            <PasswordField
              inset
              placeholder={tr('security.signIn.current')}
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
            placeholder={tr('security.signIn.newPassword', { min: PASSWORD_MIN })}
            value={password}
            onChangeText={setPassword}
            autoFocus={!has}
            maxLength={128}
            returnKeyType="go"
            onSubmitEditing={save}
          />
          <Toggle label={tr('security.revokeOthers')} value={revoke} onChange={setRevoke} />
          <ErrorText>{error}</ErrorText>
          <View style={{ flexDirection: 'row', justifyContent: 'flex-end', gap: 8 }}>
            <Pill onPress={close}>{tr('common.cancel')}</Pill>
            <Pill tone="acc" onPress={save} loading={busy} disabled={!ok}>
              {tr('common.save')}
            </Pill>
          </View>
        </>
      )}
    </>
  );
}
