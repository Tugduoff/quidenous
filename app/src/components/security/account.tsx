import { Trash2 } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text } from 'react-native';

import { ErrorText, Field, PasswordField, Pill, Row, Section, TextLink } from '@/components/auth-ui';
import { Avatar, ConfirmModal } from '@/components/ui';
import { auth, authError, type Account } from '@/lib/auth';
import { blockedAsMember, useBlockActions, useBlocks } from '@/lib/moderation';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/** People I blocked, each with "Débloquer"; nothing when there is nobody. */
export function BlockedSection() {
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
    <Section title="Bloqués">
      {list.map((b) => (
        <Row
          key={b.user_id}
          icon={<Avatar member={blockedAsMember(b)} size={32} />}
          title={b.name}
          right={
            <Pill onPress={() => run(b.user_id)} loading={busy === b.user_id} disabled={busy != null}>
              Débloquer
            </Pill>
          }
        />
      ))}
      <ErrorText>{error}</ErrorText>
    </Section>
  );
}

/** "Supprimer mon compte": confirmed with the password, or a 2FA / recovery code, when the account has one. */
export function DeleteAccount({ account }: { account: Account }) {
  const t = useTheme();
  const { leave } = useSession();
  const [open, setOpen] = useState(false);
  const [withCode, setWithCode] = useState(false);
  const [value, setValue] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const field = account.has_password && !withCode ? 'password' : account.totp ? 'code' : null;

  const close = () => {
    setOpen(false);
    setWithCode(false);
    setValue('');
    setError(null);
  };
  const submit = async () => {
    if (busy || (field && !value.trim())) return;
    setBusy(true);
    setError(null);
    try {
      await auth.deleteAccount(field === 'password' ? { password: value } : field === 'code' ? { code: value.trim() } : {});
    } catch (e) {
      setError(authError(e));
      setBusy(false);
      return;
    }
    // Every token is gone already: forget it here only (no logout call), back to the welcome screen.
    await leave();
  };

  return (
    <>
      <Pressable
        accessibilityRole="button"
        onPress={() => setOpen(true)}
        style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 8, padding: 12 }}>
        <Trash2 size={18} color="#FF3B5C" />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: '#FF3B5C' }}>Supprimer mon compte</Text>
      </Pressable>
      <ConfirmModal
        visible={open}
        title="Supprimer ton compte ?"
        message="Ton profil et tes messages sont effacés pour de bon."
        confirmLabel="Supprimer"
        busy={busy}
        confirmDisabled={!!field && !value.trim()}
        onCancel={close}
        onConfirm={submit}>
        {field === 'password' && (
          <PasswordField
            inset
            autoFocus
            placeholder="Mot de passe"
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
            placeholder="Code de l'app ou de secours"
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
            {withCode ? 'Utiliser le mot de passe' : 'Utiliser un code'}
          </TextLink>
        )}
        <ErrorText>{error}</ErrorText>
      </ConfirmModal>
    </>
  );
}
