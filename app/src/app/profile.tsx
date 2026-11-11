import { useMutation, useQueryClient } from '@tanstack/react-query';
import * as ImageManipulator from 'expo-image-manipulator';
import * as ImagePicker from 'expo-image-picker';
import { router } from 'expo-router';
import { Camera, ChevronLeft, LogOut, Moon, Sun, SunMoon, UserX } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Alert, Platform, Pressable, Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, Button, Card, Display } from '@/components/ui';
import { api, apiUpload, type Member } from '@/lib/api';
import { keys, useGroup } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { setThemePref, useTheme, useThemePref, type ThemePref } from '@/lib/theme';

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

/** Ask before voting someone out (a vote can be taken back). */
function confirm(message: string): Promise<boolean> {
  if (Platform.OS === 'web') return Promise.resolve(typeof window === 'undefined' || window.confirm(message));
  return new Promise((resolve) =>
    Alert.alert('Exclure du groupe ?', message, [
      { text: 'Annuler', style: 'cancel', onPress: () => resolve(false) },
      { text: 'Voter', style: 'destructive', onPress: () => resolve(true) },
    ]),
  );
}

/** The other members, each with a vote to remove them. A majority of the others removes someone. */
function Members() {
  const t = useTheme();
  const qc = useQueryClient();
  const { me } = useSession();
  const { data: group } = useGroup();
  const kick = useMutation({
    mutationFn: ({ id, undo }: { id: number; undo: boolean }) => api(`/members/${id}/kick`, { method: undo ? 'DELETE' : 'PUT' }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.group }),
  });
  if (!group) return null;
  const others = group.members.filter((m) => m.id !== me?.id);
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>
        {`Membres · exclure quelqu'un demande ${group.kick_needed} vote${group.kick_needed > 1 ? 's' : ''}`}
      </Text>
      {others.map((m) => {
        const state = group.kicks.find((k) => k.target_id === m.id);
        const mine = !!state?.mine;
        const onPress = async () => {
          if (!mine && !(await confirm(`Voter pour exclure ${m.name} ? Avec ${group.kick_needed} votes, ${m.name} sort du groupe.`)))
            return;
          kick.mutate({ id: m.id, undo: mine });
        };
        return (
          <Card key={m.id} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
            <Avatar member={m} size={36} />
            <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{m.name}</Text>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={mine ? `Retirer mon vote contre ${m.name}` : `Voter pour exclure ${m.name}`}
              disabled={kick.isPending}
              onPress={onPress}
              hitSlop={6}
              style={{
                flexDirection: 'row',
                alignItems: 'center',
                gap: 6,
                height: 34,
                paddingHorizontal: 12,
                borderRadius: t.radius.pill,
                backgroundColor: mine ? '#FF4FA322' : t.card2,
              }}>
              <UserX size={15} color={mine ? t.tag : t.mut} />
              <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: mine ? t.tag : t.mut }}>
                {state ? `${state.count}/${group.kick_needed}` : 'Exclure'}
              </Text>
            </Pressable>
          </Card>
        );
      })}
    </View>
  );
}

export default function Profile() {
  const t = useTheme();
  const qc = useQueryClient();
  const { me, setMe, leave } = useSession();
  const pref = useThemePref();
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

      {error && <Text style={{ fontFamily: t.font.semibold, color: t.tag }}>{error}</Text>}

      <Members />

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
