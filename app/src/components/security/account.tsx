import { Trash2 } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text } from 'react-native';
import { needsReauth, ReauthProviders } from '@/components/security/reauth';
import { auth, type Account } from '@/lib/api/auth';
import { useT } from '@/lib/i18n';
import { blockedAsMember, useBlockActions, useBlocks } from '@/lib/api/moderation';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';
import { ConfirmModal } from '@/components/ui/dialog';
import { Field, PasswordField } from '@/components/ui/field';
import { ErrorText, TextLink } from '@/components/ui/text';
import { Pill } from '@/components/ui/pill';
import { Row, Section } from '@/components/ui/section';
import { authError } from '@/lib/api/errors';

export function BlockedSection() {
  const tr = useT();
  const { list } = useBlocks();
  const { unblock } = useBlockActions();
  const [busy, setBusy] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  if (!list.length) return null;
  const run = async (userId: number) => {
    setBusy(userId);
    setError(null);
    await unblock(userId).catch((e) => setError(authError(e)));
    setBusy(null);
  };
  return (
    <Section title={tr('security.blocked.title')}>
      {list.map((b) => (
        <Row
          key={b.user_id}
          icon={<Avatar member={blockedAsMember(b)} size={32} />}
          title={b.name}
          right={
            <Pill onPress={() => run(b.user_id)} loading={busy === b.user_id} disabled={busy != null}>
              {tr('security.blocked.unblock')}
            </Pill>
          }
        />
      ))}
      <ErrorText>{error}</ErrorText>
    </Section>
  );
}

export function DeleteAccount({ account }: { account: Account }) {
  const t = useTheme();
  const tr = useT();
  const { leave } = useSession();
  const [open, setOpen] = useState(false);
  const [withCode, setWithCode] = useState(false);
  const [value, setValue] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [provider, setProvider] = useState(false);
  const field = account.has_password && !withCode ? 'password' : account.totp ? 'code' : null;

  const close = () => {
    setOpen(false);
    setWithCode(false);
    setValue('');
    setError(null);
    setProvider(false);
  };
  const submit = async () => {
    if (busy || (field && !value.trim())) return;
    setBusy(true);
    setError(null);
    try {
      await auth.deleteAccount(field === 'password' ? { password: value } : field === 'code' ? { code: value.trim() } : {});
    } catch (e) {
      if (!field && needsReauth(e)) setProvider(true);
      else setError(authError(e));
      setBusy(false);
      return;
    }
    await leave();
  };

  return (
    <>
      <Pressable
        accessibilityRole="button"
        onPress={() => setOpen(true)}
        style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 8, padding: 12 }}>
        <Trash2 size={18} color={t.danger} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.danger }}>{tr('security.deleteAccount.button')}</Text>
      </Pressable>
      <ConfirmModal
        visible={open}
        title={tr('security.deleteAccount.title')}
        message={tr('security.deleteAccount.message')}
        confirmLabel={tr('common.delete')}
        busy={busy}
        confirmDisabled={(!!field && !value.trim()) || provider}
        onCancel={close}
        onConfirm={submit}>
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
            {withCode ? tr('security.deleteAccount.usePassword') : tr('security.deleteAccount.useCode')}
          </TextLink>
        )}
        {provider && <ReauthProviders account={account} onDone={submit} />}
        <ErrorText>{error}</ErrorText>
      </ConfirmModal>
    </>
  );
}
