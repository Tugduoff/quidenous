// « Aujourd'hui »: every room of mine on one screen, what each one wants from me today, most pressing first
// (GET /api/today, lib/today.ts). A tab only with two rooms or more; tapping a card opens that room's vote tab.
import { useIsFocused } from 'expo-router';
import { useCallback, useState } from 'react';
import { RefreshControl, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { TodayCard } from '@/components/today/today-card';
import { TodayHeader } from '@/components/today/today-header';
import { TodaySkeleton } from '@/components/today/today-skeleton';
import { TodaySummary } from '@/components/today/today-summary';
import { motion, useArrivals } from '@/lib/motion';
import { useOpenRoom } from '@/lib/hooks/use-open-room';
import { useTheme } from '@/lib/theme';
import { useToday } from '@/lib/api/today';

export default function TodayScreen() {
  const focused = useIsFocused();
  const today = useToday(focused);
  const rooms = today.data?.rooms;
  const [pulled, setPulled] = useState(false);
  const t = useTheme();
  const refresh = useCallback(() => {
    setPulled(true);
    today.refetch().finally(() => setPulled(false));
  }, [today]);
  const open = useOpenRoom();
  const arrived = useArrivals(rooms?.map((r) => r.id));

  return (
    <Screen refreshControl={<RefreshControl refreshing={pulled} onRefresh={refresh} tintColor={t.mut} colors={[t.accInk]} progressBackgroundColor={t.card} />}>
      <TodayHeader />
      {rooms ? (
        <>
          <TodaySummary rooms={rooms} />
          <View style={{ gap: 12 }}>
            {rooms.map((r, i) => (
              <Animated.View key={r.id} layout={motion.layout} entering={arrived(r.id) ? motion.rise : motion.stagger(i)}>
                <TodayCard room={r} onOpen={open} />
              </Animated.View>
            ))}
          </View>
        </>
      ) : today.isError ? (
        <QueryState query={today} />
      ) : (
        <TodaySkeleton />
      )}
    </Screen>
  );
}
