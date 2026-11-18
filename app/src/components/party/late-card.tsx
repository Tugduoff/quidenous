import { Hourglass } from 'lucide-react-native';
import { Text } from 'react-native';
import { Card } from '@/components/ui/card';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';


export function LateCard() {
  const t = useTheme();
  const tr = useT();
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
      <Hourglass size={20} color={t.accInk} />
      <Text numberOfLines={2} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
        {tr('party.vote.late')}
      </Text>
    </Card>
  );
}
