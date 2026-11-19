import { MessageCircle } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { useTheme } from '@/lib/theme';

export function UnreadChip({ count }: { count: number }) {
  const t = useTheme();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4, height: 22, paddingHorizontal: 8, borderRadius: 11, backgroundColor: t.card2 }}>
      <MessageCircle size={13} color={t.txt} strokeWidth={2.5} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 12, color: t.txt }}>{count > 99 ? '99+' : count}</Text>
    </View>
  );
}
