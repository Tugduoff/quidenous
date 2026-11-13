import { router } from 'expo-router';
import { Plus } from 'lucide-react-native';
import { Text } from 'react-native';
import Animated from 'react-native-reanimated';

import { IconButton, PackCard, PageHeader } from '@/components/pack-card';
import { Screen } from '@/components/screen';
import { Button, Display } from '@/components/ui';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useMyPacks } from '@/lib/packs';
import { useGroup } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

/** The packs I published, with their moderation status (in review, removed and why). */
export default function MyPacksScreen() {
  const t = useTheme();
  const tr = useT();
  const { data: packs } = useMyPacks();
  const { data: group } = useGroup();
  return (
    <Screen tabs={false}>
      <PageHeader fallback="/packs">
        <IconButton icon={Plus} label={tr('packs.create')} onPress={() => router.push('/pack/edit')} />
      </PageHeader>
      <Display size={30}>{tr('packs.mine')}</Display>
      {packs?.length === 0 && (
        <>
          <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut, textAlign: 'center', marginTop: 24 }}>{tr('packs.mineNone')}</Text>
          <Button onPress={() => router.push('/pack/edit')}>{tr('packs.create')}</Button>
        </>
      )}
      {packs?.map((p, i) => (
        <Animated.View key={p.id} entering={motion.stagger(i)} layout={motion.layout}>
          <PackCard pack={p} roomLang={group?.language} withStatus />
        </Animated.View>
      ))}
    </Screen>
  );
}
