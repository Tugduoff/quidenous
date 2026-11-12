import { useQueryClient } from '@tanstack/react-query';
import * as ImageManipulator from 'expo-image-manipulator';
import * as ImagePicker from 'expo-image-picker';
import { router } from 'expo-router';
import { Camera, ChevronLeft, ChevronRight, LogOut, Moon, Sun, SunMoon, Users } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Platform, Pressable, Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, Button, Card, Display } from '@/components/ui';
import { api, apiUpload, type Member } from '@/lib/api';
import { keys } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { ACCENTS, setAccent, setThemePref, useAccent, useTheme, useThemePref, type Accent, type ThemePref } from '@/lib/theme';

const PHOTO_WIDTH = 512;
const THEMES: { pref: ThemePref; Icon: typeof Sun; label: string }[] = [
  { pref: 'system', Icon: SunMoon, label: 'Automatique' },
  { pref: 'light', Icon: Sun, label: 'Clair' },
  { pref: 'dark', Icon: Moon, label: 'Sombre' },
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
    form.append('photo', { uri: small.uri, name: 'photo.jpg', type: 'image/jpeg' } as unknown as Blob);
  }
  return form;
}

export default function Profile() {
  const t = useTheme();
  const qc = useQueryClient();
  const { me, setMe, leave } = useSession();
  const pref = useThemePref();
  const accent = useAccent();
  const [name, setName] = useState(me?.name ?? '');
  const [busy, setBusy] = useState<'photo' | 'name' | null>(null);
  const [error, setError] = useState<string | null>(null);

  const updated = (m: Member) => {
    setMe(m);
    qc.invalidateQueries({ queryKey: keys.group });
  };

  const pickPhoto = async () => {
    const picked = await ImagePicker.launchImageLibraryAsync({
      mediaTypes: ['images'],
      allowsEditing: true,
      aspect: [1, 1],
      quality: 1,
    });
    if (picked.canceled || !picked.assets[0]) return;
    setBusy('photo');
    setError(null);
    try {
      updated(await apiUpload<Member>('/me/photo', await photoForm(picked.assets[0].uri)));
    } catch {
      setError('Photo non envoyée');
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
      setError('Prénom invalide');
    } finally {
      setBusy(null);
    }
  };

  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => router.back()}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <View style={{ alignItems: 'center', gap: 12, marginVertical: 8 }}>
        <Pressable accessibilityRole="button" accessibilityLabel="Changer la photo" onPress={pickPhoto}>
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
            {busy === 'photo' ? <ActivityIndicator color={t.accTxt} /> : <Camera size={18} color={t.accTxt} />}
          </View>
        </Pressable>
        <Display size={28}>{me?.name}</Display>
      </View>

      <Card style={{ flexDirection: 'row', alignItems: 'center', paddingRight: 8 }}>
        <TextInput
          accessibilityLabel="Prénom"
          value={name}
          onChangeText={setName}
          maxLength={24}
          returnKeyType="done"
          onSubmitEditing={() => name.trim() && name.trim() !== me?.name && saveName()}
          style={{ flex: 1, padding: 16, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}
        />
        <Button
          onPress={saveName}
          loading={busy === 'name'}
          disabled={!name.trim() || name.trim() === me?.name}
          style={{ height: 44, paddingHorizontal: 18 }}>
          OK
        </Button>
      </Card>

      <View style={{ flexDirection: 'row', gap: 10 }}>
        {THEMES.map(({ pref: p, Icon, label }) => {
          const on = p === pref;
          return (
            <Pressable
              key={p}
              accessibilityRole="radio"
              accessibilityLabel={label}
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
              accessibilityLabel={`Couleur ${ACCENTS[k].label}`}
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


      <Pressable accessibilityRole="button" onPress={() => router.push('/friends')}>
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
          <Users size={20} color={t.txt} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Mes amis et mon code ami</Text>
          <ChevronRight size={20} color={t.mut} />
        </Card>
      </Pressable>

      <View style={{ flex: 1 }} />
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Se déconnecter"
        onPress={() => leave().then(() => router.replace('/join'))}
        style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 8, padding: 12 }}>
        <LogOut size={18} color={t.mut} />
      </Pressable>
    </Screen>
  );
}
