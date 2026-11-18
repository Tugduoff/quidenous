import { useMemo, useState } from 'react';
import { ActivityIndicator, SectionList, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { HistoryCard } from '@/components/history/history-card';
import { HistoryEmpty } from '@/components/history/history-empty';
import { HistoryHeader } from '@/components/history/history-header';
import { SectionLabel } from '@/components/ui/section-label';
import { QueryState } from '@/components/ui/query-state';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { useMembers } from '@/lib/api/group';
import { type HistoryItem, useHistory, usePinned } from '@/lib/api/history';
import { useTimeout } from '@/lib/hooks/use-timeout';
import { byDay } from '@/lib/utils/history';

type Section = { key: string; title: string; data: HistoryItem[] };

export default function HistoryScreen() {
  const t = useTheme();
  const tr = useT();
  const history = useHistory();
  const { data: pinned } = usePinned();
  const data = useMemo(() => history.data?.pages.flat(), [history.data]);
  const { byId } = useMembers();
  const [intro, setIntro] = useState(true);
  const loaded = !!data;
  useTimeout(() => setIntro(false), loaded ? 900 : null);
  const more = () => {
    if (history.hasNextPage && !history.isFetchingNextPage) history.fetchNextPage();
  };
  const pinnedIds = useMemo(() => (pinned ? new Set(pinned.map((i) => i.id)) : null), [pinned]);
  const sections = useMemo<Section[]>(() => {
    const days = byDay(data ?? []).map((d) => ({ key: d.key, title: d.label, data: d.items.map((i) => (pinnedIds ? { ...i, pinned: pinnedIds.has(i.id) } : i)) }));
    return pinned?.length ? [{ key: 'pinned', title: tr('history.pinned'), data: pinned }, ...days] : days;
  }, [data, pinned, pinnedIds, tr]);
  return (
    <SafeAreaView edges={['top']} style={{ flex: 1, backgroundColor: t.bg }}>
      <SectionList
        sections={sections}
        keyExtractor={(item) => String(item.id)}
        stickySectionHeadersEnabled={false}
        contentContainerStyle={{ padding: 20, flexGrow: 1 }}
        onEndReached={more}
        onEndReachedThreshold={1}
        initialNumToRender={12}
        windowSize={7}
        ItemSeparatorComponent={() => <View style={{ height: 10 }} />}
        SectionSeparatorComponent={() => <View style={{ height: 6 }} />}
        ListHeaderComponent={<HistoryHeader canExport={!!(data?.length || pinned?.length)} />}
        renderSectionHeader={({ section }) => (
          <View style={{ paddingTop: 10 }}>
            <SectionLabel>{section.title}</SectionLabel>
          </View>
        )}
        renderItem={({ item, index }) => (
          <HistoryCard item={item} byId={byId} order={intro && index < 6 ? index : undefined} />
        )}
        ListFooterComponent={
          <View>
            <QueryState query={history} />
            {data && data.length === 0 && !pinned?.length && <HistoryEmpty />}
            {history.isFetchingNextPage && <ActivityIndicator color={t.mut} style={{ paddingVertical: 8 }} />}
          </View>
        }
      />
    </SafeAreaView>
  );
}
