import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Flame, Library, Plus, Search, ShieldCheck, X } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { Chip, IconButton, PackCard, PageHeader } from '@/components/pack-card';
import { Screen } from '@/components/screen';
import { Card, Display } from '@/components/ui';
import { api } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { type CommunityMe, PACK_TAGS, type PackSort, type PackTag, packKeys, TAG_ICONS, useCommunityMe, usePackList } from '@/lib/packs';
import { useGroup } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const SORTS: PackSort[] = ['trending', 'new', 'liked', 'featured'];

/** Community packs: sort, filter by category, search; my packs, publish, moderation (admins). */
export default function PacksScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const [q, setQ] = useState('');
  const [sort, setSort] = useState<PackSort>('trending');
  const [tag, setTag] = useState<PackTag | null>(null);
  const { data: packs } = usePackList(sort, tag, q.trim());
  const { data: group } = useGroup();
  const { data: me } = useCommunityMe();
  const roomSpicy = group?.settings.vibe === 'pimente';
  const spicyOn = roomSpicy || !!me?.show_spicy;
  const spicy = useMutation({
    mutationFn: (show_spicy: boolean) => api<CommunityMe>('/community/me', { method: 'PUT', json: { show_spicy } }),
    onMutate: (show_spicy) => {
      qc.setQueryData<CommunityMe>(packKeys.me, (m) => m && { ...m, show_spicy });
      if (!show_spicy && tag === 'pimente') setTag(null);
    },
    onSuccess: (m) => qc.setQueryData(packKeys.me, m),
    onError: () => qc.invalidateQueries({ queryKey: packKeys.me }),
    onSettled: () => qc.invalidateQueries({ queryKey: ['packs', 'list'] }),
  });
  // The Pimenté category only lists something once 16+ packs are shown.
  const tags = PACK_TAGS.filter((id) => id !== 'pimente' || spicyOn);

  return (
    <Screen tabs={false}>
      <PageHeader fallback="/questions">
        {me?.admin && <IconButton icon={ShieldCheck} label={tr('admin.title')} onPress={() => router.push('/admin')} />}
        <IconButton icon={Library} label={tr('packs.mine')} onPress={() => router.push('/packs-mine')} />
        <IconButton icon={Plus} label={tr('packs.create')} onPress={() => router.push('/pack/edit')} />
      </PageHeader>
      <Display size={30}>{tr('packs.title')}</Display>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, paddingHorizontal: 14 }}>
        <Search size={18} color={t.mut} />
        <TextInput
          accessibilityLabel={tr('packs.search')}
          placeholder={tr('packs.search')}
          placeholderTextColor={t.mut}
          value={q}
          onChangeText={setQ}
          returnKeyType="search"
          style={{ flex: 1, minWidth: 0, height: 44, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
        />
        {!!q && (
          <Pressable accessibilityRole="button" accessibilityLabel={tr('common.close')} hitSlop={8} onPress={() => setQ('')}>
            <X size={18} color={t.mut} />
          </Pressable>
        )}
      </Card>

      <SortTabs value={sort} onChange={setSort} />

      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        style={{ marginHorizontal: -20, flexGrow: 0 }}
        contentContainerStyle={{ paddingHorizontal: 20, gap: 8 }}>
        {tags.map((id) => (
          <Chip key={id} icon={TAG_ICONS[id]} label={tr(`packs.tags.${id}`)} on={tag === id} onPress={() => setTag(tag === id ? null : id)} />
        ))}
        {!roomSpicy && me && <Chip icon={Flame} label={tr('packs.spicy')} on={me.show_spicy} hot onPress={() => spicy.mutate(!me.show_spicy)} />}
      </ScrollView>

      {packs?.length === 0 && (
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut, textAlign: 'center', marginTop: 24 }}>{tr('packs.none')}</Text>
      )}
      {packs?.map((p, i) => (
        <Animated.View key={p.id} entering={motion.stagger(i)} layout={motion.layout}>
          <PackCard pack={p} roomLang={group?.language} />
        </Animated.View>
      ))}
    </Screen>
  );
}

/** Trending / new / most liked / featured, as equal pills in one track. */
function SortTabs({ value, onChange }: { value: PackSort; onChange: (s: PackSort) => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View
      accessibilityRole="radiogroup"
      accessibilityLabel={tr('packs.sort.label')}
      style={{ flexDirection: 'row', padding: 3, gap: 2, borderRadius: t.radius.pill, backgroundColor: t.card2 }}>
      {SORTS.map((s) => {
        const on = s === value;
        return (
          <Pressable
            key={s}
            accessibilityRole="radio"
            accessibilityState={{ selected: on }}
            onPress={() => !on && onChange(s)}
            style={{
              flex: 1,
              height: 32,
              paddingHorizontal: 4,
              borderRadius: t.radius.pill,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: on ? t.acc : 'transparent',
            }}>
            <Text
              numberOfLines={1}
              adjustsFontSizeToFit
              minimumFontScale={0.8}
              style={{ fontFamily: t.font.semibold, fontSize: 13, color: on ? t.accTxt : t.mut }}>
              {tr(`packs.sort.${s}`)}
            </Text>
          </Pressable>
        );
      })}
    </View>
  );
}
