import { ChevronRight, type LucideIcon } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';

export function ProfileNavRow({ icon: Icon, label, dot, onPress }: { icon: LucideIcon; label: string; dot?: boolean; onPress: () => void }) {
  const t = useTheme();
  return (
    <Pressable accessibilityRole="button" onPress={onPress}>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
        <Icon size={20} color={t.txt} />
        <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{label}</Text>
        {dot && <View style={{ width: 8, height: 8, borderRadius: 4, backgroundColor: t.tag }} />}
        <ChevronRight size={20} color={t.mut} />
      </Card>
    </Pressable>
  );
}
