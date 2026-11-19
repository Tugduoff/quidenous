import { useState, useSyncExternalStore } from 'react';
import { View } from 'react-native';
import { BrandIcon } from '@/components/auth/brand-icons';
import { ApiError } from '@/lib/api/client';
import { auth, isReauth, PROVIDER_NAMES, type Account, type Provider } from '@/lib/api/auth';
import { useT } from '@/lib/i18n';
import { oauth } from '@/lib/sign-in';
import { Dialog, DialogActions, DialogTitle } from '@/components/ui/dialog';
import { Field, PasswordField } from '@/components/ui/field';
import { SoftButton } from '@/components/ui/soft-button';
import { ErrorText, TextLink } from '@/components/ui/text';
import { Pill } from '@/components/ui/pill';
import { authError } from '@/lib/api/errors';
import { useFeatures } from '@/lib/api/features';

type Ask = { account: Account; resolve: (ok: boolean) => void };
let asking: Ask | null = null;
const listeners = new Set<() => void>();
const setAsking = (next: Ask | null) => {
  asking = next;
  listeners.forEach((l) => l());
};

function askReauth(account: Account): Promise<boolean> {
  asking?.resolve(false);
  return new Promise((resolve) => setAsking({ account, resolve }));
}

export const needsReauth = (e: unknown) => e instanceof ApiError && e.code === 'reauth_required';

export async function withReauth<T>(account: Account, f: () => Promise<T>): Promise<T | null> {
  try {
    return await f();
  } catch (e) {
    if (!needsReauth(e)) throw e;
    if (!(await askReauth(account))) return null;
    return f();
  }
}

export function ReauthHost() {
  const ask = useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => asking,
  );
  const answer = (ok: boolean) => {
    ask?.resolve(ok);
    setAsking(null);
  };
  return (
    <Dialog visible={!!ask} onClose={() => answer(false)}>
      {ask && <ReauthForm account={ask.account} onDone={answer} />}
    </Dialog>
  );
}

function ReauthForm({ account, onDone }: { account: Account; onDone: (ok: boolean) => void }) {
  const tr = useT();
  const [withCode, setWithCode] = useState(false);
  const [value, setValue] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const field = account.has_password && !withCode ? 'password' : account.totp ? 'code' : null;

  const submit = async () => {
    if (!field || busy || !value.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await auth.reauth(field === 'password' ? { password: value } : { code: value.trim() });
      onDone(true);
    } catch (e) {
      setError(authError(e));
      setValue('');
      setBusy(false);
    }
  };

  return (
    <>
      <DialogTitle
        title={tr('security.reauth.title')}
        message={
          field === 'password' ? tr('security.reauth.password') : field === 'code' ? tr('security.reauth.code') : tr('security.reauth.provider')
        }
      />
      {field === 'password' && (
        <PasswordField
          inset
          autoFocus
          placeholder={tr('security.signIn.password')}
          value={value}
          onChangeText={setValue}
          returnKeyType="go"
          onSubmitEditing={submit}
        />
      )}
      {field === 'code' && (
        <Field
          inset
          autoFocus
          placeholder={tr('security.appOrRecoveryCode')}
          value={value}
          onChangeText={setValue}
          autoCapitalize="none"
          autoCorrect={false}
          autoComplete="one-time-code"
          maxLength={16}
          returnKeyType="go"
          onSubmitEditing={submit}
        />
      )}
      {account.has_password && account.totp && (
        <TextLink
          onPress={() => {
            setWithCode((c) => !c);
            setValue('');
            setError(null);
          }}
          style={{ alignSelf: 'flex-start', paddingHorizontal: 4 }}>
          {withCode ? tr('security.reauth.usePassword') : tr('security.reauth.useCode')}
        </TextLink>
      )}
      {field ? (
        <>
          <ErrorText>{error}</ErrorText>
          <DialogActions
            tone="acc"
            confirmLabel={tr('security.reauth.confirm')}
            onConfirm={submit}
            onCancel={() => onDone(false)}
            busy={busy}
            disabled={!value.trim()}
          />
        </>
      ) : (
        <>
          <ReauthProviders account={account} onDone={() => onDone(true)} />
          <View style={{ alignItems: 'flex-end' }}>
            <Pill onPress={() => onDone(false)}>{tr('common.cancel')}</Pill>
          </View>
        </>
      )}
    </>
  );
}

export function ReauthProviders({ account, onDone }: { account: Account; onDone: () => void }) {
  const tr = useT();
  const { providers: enabled } = useFeatures();
  const providers = account.providers.filter((p) => enabled.includes(p));
  const [busy, setBusy] = useState<Provider | null>(null);
  const [error, setError] = useState<string | null>(null);
  const go = async (p: Provider) => {
    setBusy(p);
    setError(null);
    try {
      const r = await oauth(p, 'reauth');
      if (r && isReauth(r)) return onDone();
    } catch (e) {
      setError(authError(e));
    }
    setBusy(null);
  };
  return (
    <>
      {providers.map((p) => (
        <SoftButton key={p} icon={<BrandIcon provider={p} />} loading={busy === p} disabled={!!busy} onPress={() => go(p)}>
          {tr('auth.welcome.continueWith', { provider: PROVIDER_NAMES[p] })}
        </SoftButton>
      ))}
      <ErrorText>{error}</ErrorText>
    </>
  );
}
