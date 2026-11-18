import { useQueryClient } from '@tanstack/react-query';
import * as ImagePicker from 'expo-image-picker';
import { router } from 'expo-router';
import { Bell, Camera, Check, ChevronRight, Images, Languages, type LucideIcon, LogOut, Moon, Pencil, ShieldCheck, Smile, Sun, SunMoon, Users } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Linking, Platform, Pressable, Text, TextInput, View } from 'react-native';
import { MyLinksRow } from '@/components/share/my-links';
import { Screen } from '@/components/ui/screen';
import { MyStreak } from '@/components/streaks/streaks';
import { useSignOut } from '@/components/security/devices';
import { BadgesRow, MyTitle } from '@/components/badges/title-chip';
import { VersionLine } from '@/components/update/update';
import { WrappedRow } from '@/components/wrapped/entry';
import { API_URL, ApiError } from '@/lib/api/client';
import { useAccount } from '@/lib/api/auth';
import { useDonateUrl } from '@/lib/api/donate';
import { type LangPref, setLangPref, type TKey, useLang, useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { ACCENTS, setAccent, setThemePref, useAccent, useTheme, useThemePref, type Accent, type ThemePref } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Display } from '@/components/ui/text';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import { BackButton } from '@/components/ui/back-button';
import { type Member, groupKeys } from '@/lib/api/group';
import { uploadMyPhoto, setMyName } from '@/lib/api/me';
import { photoForm } from '@/lib/device/photo';
import { useFeatures } from '@/lib/api/features';


const THEMES: { pref: ThemePref; Icon: typeof Sun; label: TKey }[] = [
  { pref: 'system', Icon: SunMoon, label: 'profile.theme.system' },
  { pref: 'light', Icon: Sun, label: 'profile.theme.light' },
  { pref: 'dark', Icon: Moon, label: 'profile.theme.dark' },
];
/** Profile > Langue: follow the phone, or a fixed language (each named in its own language). */
const LANGS: { pref: LangPref; label: TKey }[] = [
  { pref: 'auto', label: 'profile.language.auto' },
  { pref: 'fr', label: 'profile.language.fr' },
  { pref: 'en', label: 'profile.language.en' },
];


function Choice({ icon: Icon, label, onPress }: { icon: LucideIcon; label: string; onPress: () => void }) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      onPress={onPress}
      style={({ pressed }) => ({
        flexDirection: 'row',
        alignItems: 'center',
        gap: 12,
        height: 52,
        paddingHorizontal: 16,
        borderRadius: 16,
        backgroundColor: t.card2,
        opacity: pressed ? 0.7 : 1,
      })}>
      <Icon size={20} color={t.txt} />
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>{label}</Text>
      <ChevronRight size={18} color={t.mut} />
    </Pressable>
  );
}

/** One of the language choices: a row, ticked when it's the current one. */
function LangOption({ label, on, onPress }: { label: string; on: boolean; onPress: () => void }) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="radio"
      accessibilityState={{ selected: on }}
      onPress={onPress}
      style={({ pressed }) => ({
        flexDirection: 'row',
        alignItems: 'center',
        gap: 12,
        height: 52,
        paddingHorizontal: 16,
        borderRadius: 16,
        borderWidth: 2,
        borderColor: on ? t.accInk : 'transparent',
        backgroundColor: on ? t.accSoft : t.card2,
        opacity: pressed ? 0.7 : 1,
      })}>
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: on ? t.accInk : t.txt }}>{label}</Text>
      {on && <Check size={18} color={t.accInk} />}
    </Pressable>
  );
}

export default function Profile() {
  const t = useTheme();
  const tr = useT();
  const { lang, pref: langPref } = useLang();
  const [langOpen, setLangOpen] = useState(false);
  const qc = useQueryClient();
  const { me, setMe } = useSession();
  const features = useFeatures();
  const { data: account } = useAccount();
  const signOut = useSignOut();
  const pref = useThemePref();
  const accent = useAccent();
  const [name, setName] = useState(me?.name ?? '');
  const [busy, setBusy] = useState<'photo' | 'name' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [choosing, setChoosing] = useState(false);

  const updated = (m: Member) => {
    setMe(m);
    qc.invalidateQueries({ queryKey: groupKeys.all });
  };

  const pickPhoto = async (from: 'library' | 'camera') => {
    setChoosing(false);
    setError(null);
    try {
      const options: ImagePicker.ImagePickerOptions = { mediaTypes: ['images'], allowsEditing: true, aspect: [1, 1], quality: 1 };
      if (from === 'camera' && Platform.OS !== 'web' && !(await ImagePicker.requestCameraPermissionsAsync()).granted) {
        setError(tr('profile.photo.cameraDenied'));
        return;
      }
      const picked = await (from === 'camera' ? ImagePicker.launchCameraAsync(options) : ImagePicker.launchImageLibraryAsync(options));
      if (picked.canceled || !picked.assets[0]) return;
      setBusy('photo');
      updated(await uploadMyPhoto(await photoForm(picked.assets[0].uri)));
    } catch (e) {
      // Say why, so a failure on someone's phone can be told apart (server refusal vs. on the phone).
      setError(
        tr('profile.photo.failed', {
          reason: e instanceof ApiError ? e.status : tr('profile.photo.error'),
        }),
      );
    } finally {
      setBusy(null);
    }
  };

  const saveName = async () => {
    setBusy('name');
    setError(null);
    try {
      updated(await setMyName(name));
    } catch {
      setError(tr('profile.badName'));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Screen tabs={false}>
      <BackButton onPress={() => router.back()} />
      {/* No room yet: the photo and the name live on room seats, so only the account's name shows. */}
      {!me && account && (
        <View style={{ alignItems: 'center', marginVertical: 8 }}>
          <Display size={account.name.length > 14 ? 22 : 28}>{account.name}</Display>
        </View>
      )}
      {me && (
        <View style={{ alignItems: 'center', gap: 12, marginVertical: 8 }}>
          <Pressable accessibilityRole="button" accessibilityLabel={tr('profile.photo.change')} onPress={() => setChoosing(true)}>
            <Avatar member={me} size={112} />
            <View
              style={{
                position: 'absolute',
                right: 0,
                bottom: 0,
                width: 36,
                height: 36,
                borderRadius: 18,
                backgroundColor: t.acc,
                alignItems: 'center',
                justifyContent: 'center',
                borderWidth: 3,
                borderColor: t.bg,
              }}>
              {busy === 'photo' ? <ActivityIndicator color={t.accTxt} /> : <Pencil size={16} color={t.accTxt} />}
            </View>
          </Pressable>
          <Display size={me.name.length > 14 ? 22 : 28}>{me.name}</Display>
          <MyTitle />
          <MyStreak />
        </View>
      )}

      <Dialog visible={choosing} onClose={() => setChoosing(false)}>
        <DialogTitle title={tr('profile.photo.title')} />
        {features.avatars && (
          <Choice
            icon={Smile}
            label={tr('profile.photo.avatar')}
            onPress={() => {
              setChoosing(false);
              router.push('/avatar');
            }}
          />
        )}
        <Choice icon={Images} label={tr('profile.photo.library')} onPress={() => pickPhoto('library')} />
        <Choice icon={Camera} label={tr('profile.photo.camera')} onPress={() => pickPhoto('camera')} />
      </Dialog>

      {me && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', paddingRight: 8 }}>
          <TextInput
            accessibilityLabel={tr('profile.name')}
            value={name}
            onChangeText={setName}
            maxLength={24}
            returnKeyType="done"
            onSubmitEditing={() => name.trim() && name.trim() !== me?.name && saveName()}
            style={{ flex: 1, minWidth: 0, padding: 16, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}
          />
          <Button
            onPress={saveName}
            loading={busy === 'name'}
            disabled={!name.trim() || name.trim() === me?.name}
            style={{ height: 44, paddingHorizontal: 18 }}>
            {tr('profile.saveName')}
          </Button>
        </Card>
      )}

      <View style={{ flexDirection: 'row', gap: 10 }}>
        {THEMES.map(({ pref: p, Icon, label }) => {
          const on = p === pref;
          return (
            <Pressable
              key={p}
              accessibilityRole="radio"
              accessibilityLabel={tr(label)}
              accessibilityState={{ selected: on }}
              onPress={() => setThemePref(p)}
              style={[
                { flex: 1, height: 56, borderRadius: 18, alignItems: 'center', justifyContent: 'center', borderWidth: 2 },
                { backgroundColor: on ? t.accSoft : t.card, borderColor: on ? t.accInk : 'transparent' },
                t.shadow,
              ]}>
              <Icon size={22} color={on ? t.accInk : t.mut} />
            </Pressable>
          );
        })}
      </View>

      <View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
        {(Object.keys(ACCENTS) as Accent[]).map((k) => {
          const on = k === accent;
          return (
            <Pressable
              key={k}
              accessibilityRole="radio"
              accessibilityLabel={tr('profile.accentA11y', { name: tr(`profile.accents.${k}`) })}
              accessibilityState={{ selected: on }}
              onPress={() => setAccent(k)}
              style={{
                width: 44,
                height: 44,
                borderRadius: 22,
                alignItems: 'center',
                justifyContent: 'center',
                borderWidth: 2,
                borderColor: on ? t.txt : 'transparent',
              }}>
              <View style={{ width: 32, height: 32, borderRadius: 16, backgroundColor: ACCENTS[k].acc }} />
            </Pressable>
          );
        })}
      </View>

      {error && <Text style={{ fontFamily: t.font.semibold, color: t.tagInk }}>{error}</Text>}

      <Pressable accessibilityRole="button" onPress={() => setLangOpen(true)}>
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
          <Languages size={20} color={t.txt} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('profile.language.title')}</Text>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr((LANGS.find((l) => l.pref === langPref) ?? LANGS[0]).label)}</Text>
          <ChevronRight size={20} color={t.mut} />
        </Card>
      </Pressable>
      <Dialog visible={langOpen} onClose={() => setLangOpen(false)}>
        <DialogTitle title={tr('profile.language.title')} />
        {LANGS.map(({ pref: p, label }) => (
          <LangOption
            key={p}
            label={tr(label)}
            on={p === langPref}
            onPress={() => {
              setLangOpen(false);
              // Server texts (stats stories, inbox) follow the account's language: refetch once it knows.
              if (p !== langPref)
                setLangPref(p).then(
                  () => qc.invalidateQueries(),
                  () => {},
                );
            }}
          />
        ))}
      </Dialog>

      <BadgesRow />
      <WrappedRow />

      {!account?.guest && (
        <Pressable accessibilityRole="button" onPress={() => router.push('/friends')}>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            <Users size={20} color={t.txt} />
            <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('profile.friends')}</Text>
            <ChevronRight size={20} color={t.mut} />
          </Card>
        </Pressable>
      )}

      {features.notificationSettings && (
        <Pressable accessibilityRole="button" onPress={() => router.push('/notifications')}>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            <Bell size={20} color={t.txt} />
            <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('profile.notifications')}</Text>
            <ChevronRight size={20} color={t.mut} />
          </Card>
        </Pressable>
      )}

      <MyLinksRow />

      {features.auth && (
        <Pressable accessibilityRole="button" onPress={() => router.push('/security')}>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            <ShieldCheck size={20} color={t.txt} />
            <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('profile.security')}</Text>
            {account && !account.secured && <View style={{ width: 8, height: 8, borderRadius: 4, backgroundColor: t.tag }} />}
            <ChevronRight size={20} color={t.mut} />
          </Card>
        </Pressable>
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

/** "Buy us a coffee": a quiet link to the donation page in the browser, when there is one (lib/donate.ts). */
function DonateLink() {
  const t = useTheme();
  const tr = useT();
  const url = useDonateUrl();
  if (!url) return null;
  return (
    <View style={{ alignItems: 'center', gap: 2, paddingHorizontal: 24 }}>
      <Pressable accessibilityRole="link" onPress={() => Linking.openURL(url).catch(() => {})} hitSlop={8}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.txt }}>{tr('profile.donate.title')}</Text>
      </Pressable>
      <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut, textAlign: 'center' }}>{tr('profile.donate.line')}</Text>
    </View>
  );
}

/** Discreet footer: the privacy policy and the terms, in the app's language. */
function LegalLinks({ lang }: { lang: 'fr' | 'en' }) {
  const t = useTheme();
  const tr = useT();
  const open = (page: 'privacy' | 'terms') => Linking.openURL(`${API_URL}${lang === 'en' ? '/en' : ''}/${page}`).catch(() => {});
  const small = { fontFamily: t.font.semibold, fontSize: 12, color: t.mut };
  return (
    <View style={{ flexDirection: 'row', justifyContent: 'center', alignItems: 'center', gap: 6 }}>
      <Pressable accessibilityRole="link" onPress={() => open('privacy')} hitSlop={8}>
        <Text style={small}>{tr('legal.privacy')}</Text>
      </Pressable>
      <Text style={small}>·</Text>
      <Pressable accessibilityRole="link" onPress={() => open('terms')} hitSlop={8}>
        <Text style={small}>{tr('legal.terms')}</Text>
      </Pressable>
    </View>
  );
}
