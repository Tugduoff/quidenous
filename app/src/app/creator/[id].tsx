import { useLocalSearchParams } from 'expo-router';
import { Download, Heart } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { PackCard, PageHeader, Stat } from '@/components/packs/pack-card';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { isNotFound } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useCreator } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { Avatar } from '@/components/ui/avatar';
import { Display } from '@/components/ui/text';
import { useGroup } from '@/lib/api/group';

/** Someone who publishes packs: their face, name, installs and likes over all their packs, and the packs. */
export default function CreatorScreen() {
  const t = useTheme();
  const tr = useT();
  const id = Number(useLocalSearchParams<{ id: string }>().id);
  const query = useCreator(id);
  const creator = query.data;
  const { data: group } = useGroup();
  return (
    <Screen tabs={false}>
      <PageHeader fallback="/packs" />
      {isNotFound(query.error) ? (
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>{tr('packs.creatorNotFound')}</Text>
      ) : (
        <QueryState query={query} />
      )}
      {creator && (
        <>
          <Animated.View entering={motion.enter} style={{ flexDirection: 'row', alignItems: 'center', gap: 14 }}>
            <Avatar member={creator} size={64} />
            <View style={{ flex: 1, gap: 6 }}>
              <Display size={26}>{creator.name}</Display>
              <View style={{ flexDirection: 'row', gap: 14 }}>
                <Stat icon={Download} value={creator.installs} label={tr('packs.installs', { count: creator.installs })} />
                <Stat icon={Heart} value={creator.likes} label={tr('packs.likes', { count: creator.likes })} />
              </View>
            </View>
          </Animated.View>
          {creator.packs.length > 0 && (
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4, marginTop: 4 }}>{tr('packs.creatorPacks')}</Text>
          )}
          {creator.packs.map((p, i) => (
            <Animated.View key={p.id} entering={motion.stagger(i)}>
              <PackCard pack={p} roomLang={group?.language} />
            </Animated.View>
          ))}
        </>
      )}
    </Screen>
  );
}
