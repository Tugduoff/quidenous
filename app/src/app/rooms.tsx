import { router } from 'expo-router';
import { Check, ChevronLeft, ChevronRight, Crown } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';

import { RoomForms } from '@/components/room-forms';
import { Screen } from '@/components/screen';
import { Card, Display } from '@/components/ui';
import { presetJoinCode } from '@/lib/api';
import { useRooms } from '@/lib/queries';
import { useSwitchRoom } from '@/lib/rooms';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

/** All my rooms, to switch between them, plus create one or join one with a code. */
export default function RoomsScreen() {
  const t = useTheme();
  const { room } = useSession();
  const { data: rooms } = useRooms();
  const switchRoom = useSwitchRoom();
  const current = room ?? rooms?.[0]?.id;
  // Picking a new room's vibe: only that on screen.
  const [creating, setCreating] = useState(false);

  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      {!creating && <Display size={30}>Mes salons</Display>}

      {!creating && rooms?.map((r) => {
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

      <RoomForms initialCode={presetJoinCode() ?? ''} onEntered={switchRoom} onVibeStep={setCreating} />
    </Screen>
  );
}
