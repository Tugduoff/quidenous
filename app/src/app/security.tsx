import { ActivityIndicator } from 'react-native';
import { Screen } from '@/components/ui/screen';
import { BlockedSection, DeleteAccount } from '@/components/security/account';
import { LockSection, ProvidersSection, SessionsSection, SignOutButton } from '@/components/security/devices';
import { ReauthHost } from '@/components/security/reauth';
import { SignInSection } from '@/components/security/sign-in';
import { TwoFactorSection } from '@/components/security/two-factor';
import { authError, useAccount } from '@/lib/auth';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Display, ErrorText } from '@/components/ui/text';
import { BackButton } from '@/components/ui/back-button';

/** Profile > Sécurité: email, password, 2FA, linked accounts, devices, blocked people, app lock, sign out, delete the account. */
export default function Security() {
  const t = useTheme();
  const tr = useT();
  const { data: account, error } = useAccount();
  return (
    <Screen tabs={false}>
      <BackButton />
      <Display size={30}>{tr('security.title')}</Display>
      {!account ? (
        error ? (
          <ErrorText>{authError(error)}</ErrorText>
        ) : (
          <ActivityIndicator color={t.mut} style={{ marginTop: 24 }} />
        )
      ) : (
        <>
          <SignInSection account={account} />
          <ProvidersSection account={account} />
          {account.secured && <TwoFactorSection account={account} />}
          <SessionsSection />
          <BlockedSection />
          <LockSection />
          <SignOutButton />
          <DeleteAccount account={account} />
          <ReauthHost />
        </>
      )}
    </Screen>
  );
}
