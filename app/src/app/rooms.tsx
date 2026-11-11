import { useMutation } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Check, ChevronLeft, ChevronRight, Crown, LogIn, Plus } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Card, Display } from '@/components/ui';
import { api, ApiError, presetJoinCode } from '@/lib/api';
import { useRooms } from '@/lib/queries';
import { useSwitchRoom } from '@/lib/rooms';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

const ERRORS: Record<string, string> = {
  bad_code: 'Code inconnu',
  removed: 'Tu as été retiré de ce salon',
  bad_room_name: 'Donne-lui un nom (40 caractères max)',
};

/** All my rooms, to switch between them, plus create one or join one with a code. */
export default function RoomsScreen() {
  const t = useTheme();
  const { room } = useSession();
  const { data: rooms } = useRooms();
  const switchRoom = useSwitchRoom();
  const [name, setName] = useState('');
  const [code, setCode] = useState(presetJoinCode() ?? '');
  const [error, setError] = useState<string | null>(null);
  const fail = (e: unknown) => setError(e instanceof ApiError ? (ERRORS[e.code] ?? 'Raté, réessaie') : 'Raté, réessaie');

  const create = useMutation({
    mutationFn: () => api<{ id: number }>('/rooms', { method: 'POST', json: { name: name.trim() } }),
    onSuccess: (r) => switchRoom(r.id),
    onError: fail,
  });
  const join = useMutation({
    mutationFn: () => api<{ id: number }>('/rooms/join', { method: 'POST', json: { code: code.trim() } }),
    onSuccess: (r) => switchRoom(r.id),
    onError: fail,
  });
  const current = room ?? rooms?.[0]?.id;

  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>Mes salons</Display>

      {rooms?.map((r) => {
        const on = r.id === current;
        const waiting = r.open_number != null && !r.voted;
        return (
          <Pressable key={r.id} accessibilityRole="button" onPress={() => (on ? router.back() : switchRoom(r.id))}>
            <Card
              style={{
                flexDirection: 'row',
                alignItems: 'center',
                gap: 12,
                padding: 16,
                borderWidth: 2,
                borderColor: on ? t.accInk : 'transparent',
              }}>
              <View style={{ flex: 1, gap: 2 }}>
                <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                  <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>{r.name}</Text>
                  {r.role === 'owner' && <Crown size={14} color={t.accInk} />}
                </View>
                <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
                  {`${r.members} membre${r.members > 1 ? 's' : ''}`}
                  {waiting ? ` · question #${r.open_number} à voter` : ''}
                </Text>
              </View>
              {r.unread > 0 && (
                <View
                  style={{
                    minWidth: 22,
                    height: 22,
                    paddingHorizontal: 6,
                    borderRadius: 11,
                    backgroundColor: t.tag,
                    alignItems: 'center',
                    justifyContent: 'center',
                  }}>
                  <Text style={{ fontFamily: t.font.bold, fontSize: 11, color: '#fff' }}>{r.unread > 9 ? '9+' : r.unread}</Text>
                </View>
              )}
              {on ? <Check size={20} color={t.accInk} /> : <ChevronRight size={20} color={t.mut} />}
            </Card>
          </Pressable>
        );
      })}

      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4, marginTop: 8 }}>Nouveau salon</Text>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16 }}>
        <TextInput
          accessibilityLabel="Nom du salon"
          placeholder="Les cousins, Le boulot…"
          placeholderTextColor={t.mut}
          value={name}
          onChangeText={setName}
          maxLength={40}
          style={{ flex: 1, height: 44, fontFamily: t.font.regular, fontSize: 15, color: t.txt }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Créer le salon"
          disabled={!name.trim() || create.isPending}
          onPress={() => create.mutate()}
          style={{
            width: 44,
            height: 44,
            borderRadius: 22,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: name.trim() ? t.acc : t.card2,
          }}>
          <Plus size={20} color={name.trim() ? t.accTxt : t.mut} />
        </Pressable>
      </Card>

      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>Rejoindre avec un code</Text>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16 }}>
        <TextInput
          accessibilityLabel="Code d'invitation"
          placeholder="ABC234"
          placeholderTextColor={t.mut}
          value={code}
          onChangeText={(v) => setCode(v.toUpperCase())}
          autoCapitalize="characters"
          maxLength={20}
          style={{ flex: 1, height: 44, fontFamily: t.font.bold, fontSize: 16, letterSpacing: 2, color: t.txt }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Rejoindre"
          disabled={!code.trim() || join.isPending}
          onPress={() => join.mutate()}
          style={{
            width: 44,
            height: 44,
            borderRadius: 22,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: code.trim() ? t.acc : t.card2,
          }}>
          <LogIn size={20} color={code.trim() ? t.accTxt : t.mut} />
        </Pressable>
      </Card>
      {error && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag, marginLeft: 4 }}>{error}</Text>}
    </Screen>
  );
}
