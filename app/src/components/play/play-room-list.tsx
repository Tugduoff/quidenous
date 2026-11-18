import { ChevronRight, Crown } from 'lucide-react-native';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { Card } from '@/components/ui/card';
import { SectionLabel } from '@/components/ui/section-label';
import type { RoomSummary } from '@/lib/api/rooms';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function PlayRoomList({
  rooms,
  pending,
  busyId,
  error,
  onPick,
}: {
  rooms: RoomSummary[];
  pending: boolean;
  busyId: number | undefined;
  error: string | null;
  onPick: (id: number) => void;
}) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 10, marginTop: 8 }}>
      <SectionLabel>{tr('community.play.pick')}</SectionLabel>
      {rooms.map((r) => {
        const busy = pending && busyId === r.id;
        return (
          <Pressable key={r.id} accessibilityRole="button" disabled={pending} onPress={() => onPick(r.id)}>
            <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
              <View style={{ flex: 1, gap: 2 }}>
                <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                  <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
                    {r.name}
                  </Text>
                  {r.role === 'owner' && <Crown size={14} color={t.accInk} />}
                </View>
                <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
                  {tr('common.members', { count: r.members })}
                </Text>
              </View>
              {busy ? <ActivityIndicator color={t.mut} /> : <ChevronRight size={20} color={t.mut} />}
            </Card>
          </Pressable>
        );
      })}
      {error && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tagInk, marginLeft: 4 }}>{error}</Text>}
    </View>
  );
}
