import { useQueryClient } from '@tanstack/react-query';
import { File as FsFile } from 'expo-file-system';
import * as ImageManipulator from 'expo-image-manipulator';
import * as ImagePicker from 'expo-image-picker';
import { router } from 'expo-router';
import {
  Bell,
  Camera,
  Check,
  ChevronLeft,
  ChevronRight,
  Images,
  Languages,
  type LucideIcon,
  LogOut,
  Moon,
  Pencil,
  ShieldCheck,
  Smile,
  Sun,
  SunMoon,
  Users,
} from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Linking, Platform, Pressable, Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { useSignOut } from '@/components/security/devices';
import { Avatar, Button, Card, Dialog, DialogTitle, Display } from '@/components/ui';
import { VersionLine } from '@/components/update';
import { api, API_URL, ApiError, apiUpload, type Member } from '@/lib/api';
import { useAccount, useFeatures } from '@/lib/auth';
import { type LangPref, setLangPref, type TKey, useLang, useT } from '@/lib/i18n';
import { keys } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { ACCENTS, setAccent, setThemePref, useAccent, useTheme, useThemePref, type Accent, type ThemePref } from '@/lib/theme';

const PHOTO_WIDTH = 512;
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

async function photoForm(uri: string) {
  const small = await ImageManipulator.manipulateAsync(uri, [{ resize: { width: PHOTO_WIDTH } }], {
    compress: 0.8,
    format: ImageManipulator.SaveFormat.JPEG,
  });
  const form = new FormData();
  if (Platform.OS === 'web') {
    form.append('photo', await (await fetch(small.uri)).blob(), 'photo.jpg');
  } else {
    // expo/fetch (the global fetch since SDK 58) can't send React Native's { uri } parts; a file-system File it can.
    form.append('photo', new FsFile(small.uri), 'photo.jpg');
  }
  return form;
}

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
    qc.invalidateQueries({ queryKey: keys.group });
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
      updated(await apiUpload<Member>('/me/photo', await photoForm(picked.assets[0].uri)));
    } catch (e) {
      // Say why, so a failure on someone's phone can be told apart (server refusal vs. on the phone).
      setError(
        tr('profile.photo.failed', {
          reason: e instanceof ApiError ? e.status : e instanceof Error ? e.message.slice(0, 60) : tr('profile.photo.error'),
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
      updated(await api<Member>('/me', { method: 'PUT', json: { name } }));
    } catch {
      setError(tr('profile.badName'));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('common.back')}
        onPress={() => router.back()}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <View style={{ alignItems: 'center', gap: 12, marginVertical: 8 }}>
        <Pressable accessibilityRole="button" accessibilityLabel={tr('profile.photo.change')} onPress={() => setChoosing(true)}>
          <Avatar member={me ?? undefined} size={112} />
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
        <Display size={(me?.name.length ?? 0) > 14 ? 22 : 28}>{me?.name}</Display>
      </View>

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

      {error && <Text style={{ fontFamily: t.font.semibold, color: t.tag }}>{error}</Text>}

      <Pressable accessibilityRole="button" onPress={() => setLangOpen(true)}>
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
          <Languages size={20} color={t.txt} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('profile.language.title')}</Text>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr(LANGS.find((l) => l.pref === langPref)!.label)}</Text>
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
              if (p !== langPref) setLangPref(p).then(() => qc.invalidateQueries(), () => {});
            }}
          />
        ))}
      </Dialog>

      <Pressable accessibilityRole="button" onPress={() => router.push('/friends')}>
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
          <Users size={20} color={t.txt} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('profile.friends')}</Text>
          <ChevronRight size={20} color={t.mut} />
        </Card>
      </Pressable>

      {features.notificationSettings && (
        <Pressable accessibilityRole="button" onPress={() => router.push('/notifications')}>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            <Bell size={20} color={t.txt} />
            <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('profile.notifications')}</Text>
            <ChevronRight size={20} color={t.mut} />
          </Card>
        </Pressable>
      )}

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
      <LegalLinks lang={lang} />
    </Screen>
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
