import { router } from 'expo-router';
import { Bell, LogOut, ShieldCheck, Users } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { MyLinksRow } from '@/components/share/my-links';
import { Screen } from '@/components/ui/screen';
import { useSignOut } from '@/components/security/devices';
import { BadgesRow } from '@/components/badges/title-chip';
import { VersionLine } from '@/components/update/update';
import { WrappedRow } from '@/components/wrapped/entry';
import { useAccount } from '@/lib/api/auth';
import { useLang, useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { BackButton } from '@/components/ui/back-button';
import { useFeatures } from '@/lib/api/features';
import { useProfileEdit } from '@/lib/hooks/use-profile-edit';
import { ProfileHeader } from '@/components/profile/profile-header';
import { PhotoSourceDialog } from '@/components/profile/photo-source-dialog';
import { NameCard } from '@/components/profile/name-card';
import { AccentPicker, ThemePicker } from '@/components/profile/appearance';
import { LanguageRow } from '@/components/profile/language-row';
import { ProfileNavRow } from '@/components/profile/profile-nav-row';
import { DonateLink } from '@/components/profile/donate-link';
import { LegalLinks } from '@/components/profile/legal-links';

export default function Profile() {
  const t = useTheme();
  const tr = useT();
  const { lang } = useLang();
  const { me } = useSession();
  const features = useFeatures();
  const { data: account } = useAccount();
  const signOut = useSignOut();
  const { name, setName, busy, error, pickPhoto, saveName } = useProfileEdit();
  const [choosing, setChoosing] = useState(false);

  return (
    <Screen tabs={false}>
      <BackButton onPress={() => router.back()} />
      <ProfileHeader me={me} accountName={account?.name} photoBusy={busy === 'photo'} onPhoto={() => setChoosing(true)} />

      <PhotoSourceDialog
        visible={choosing}
        onClose={() => setChoosing(false)}
        avatars={features.avatars}
        onPick={(from) => {
          setChoosing(false);
          pickPhoto(from);
        }}
      />

      {me && <NameCard me={me} name={name} onName={setName} saving={busy === 'name'} onSave={saveName} />}

      <ThemePicker />
      <AccentPicker />

      {error && <Text style={{ fontFamily: t.font.semibold, color: t.tagInk }}>{error}</Text>}

      <LanguageRow />

      <BadgesRow />
      <WrappedRow />

      {!account?.guest && <ProfileNavRow icon={Users} label={tr('profile.friends')} onPress={() => router.push('/friends')} />}

      {features.notificationSettings && <ProfileNavRow icon={Bell} label={tr('profile.notifications')} onPress={() => router.push('/notifications')} />}

      <MyLinksRow />

      {features.auth && (
        <ProfileNavRow icon={ShieldCheck} label={tr('profile.security')} dot={!!account && !account.secured} onPress={() => router.push('/security')} />
      )}

      <View style={{ flex: 1 }} />
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('profile.signOut')}
        onPress={signOut}
        style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 8, padding: 12 }}>
        <LogOut size={18} color={t.mut} />
      </Pressable>
      <VersionLine />
      <DonateLink />
      <LegalLinks lang={lang} />
    </Screen>
  );
}
