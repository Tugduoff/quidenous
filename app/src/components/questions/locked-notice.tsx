import { router } from 'expo-router';
import { Lightbulb, Lock, Package } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { BulkButton } from '@/components/questions/bulk-button';
import { Card } from '@/components/ui/card';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function LockedNotice({ community }: { community: boolean }) {
  const t = useTheme();
  const tr = useT();
  return (
    <>
      <Card
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          gap: 12,
          padding: 16,
        }}>
        <Lock size={18} color={t.mut} />
        <Text
          style={{
            flex: 1,
            fontFamily: t.font.regular,
            fontSize: 14,
            lineHeight: 20,
            color: t.mut,
          }}>
          {tr('questions.ownerOnly')}
        </Text>
      </Card>
      {community && (
        <View style={{ flexDirection: 'row', gap: 10 }}>
          <BulkButton label={tr('questions.packs')} icon={<Package size={16} color={t.txt} />} onPress={() => router.push('/packs')} />
          <BulkButton label={tr('community.entry')} icon={<Lightbulb size={16} color={t.txt} />} onPress={() => router.push('/suggest')} />
        </View>
      )}
    </>
  );
}
