import { useQuery } from '@tanstack/react-query';
import { Ticket } from 'lucide-react-native';
import { Text, View } from 'react-native';

import { Card } from '@/components/ui';
import { api } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

type Preview = { name: string; member_count: number; names: string[] };

/** What an invite link opens onto: the room's name, how many are in it and who. Nothing if the code is unknown (the code field says so). */
export function InvitePreview({ code }: { code: string }) {
  const t = useTheme();
  const tr = useT();
  const clean = code.trim().toUpperCase();
  const { data } = useQuery({
    queryKey: ['room-preview', clean],
    queryFn: () => api<Preview>(`/rooms/preview?code=${encodeURIComponent(clean)}`),
    enabled: clean.length >= 4,
    retry: false,
  });
  if (!data) return null;
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16, backgroundColor: t.accSoft }}>
      <Ticket size={20} color={t.accInk} />
      <View style={{ flex: 1, gap: 2 }}>
        <Text numberOfLines={2} style={{ fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
          {data.name}
        </Text>
        <Text style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
          {tr('common.members', { count: data.member_count })}
          {data.names.length > 0 ? ` · ${tr('room.invite.with', { names: data.names.join(', ') })}` : ''}
        </Text>
      </View>
    </Card>
  );
}
