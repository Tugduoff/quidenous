import { useState } from 'react';
import { Text, View } from 'react-native';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { type Badge, chipOf, useBadges } from '@/lib/api/badges';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Display } from '@/components/ui/text';
import { BackButton } from '@/components/ui/back-button';
import { useFeatures } from '@/lib/api/features';
import { BadgeTile } from '@/components/badges/badge-tile';
import { BadgeDetail } from '@/components/badges/badge-detail';
import { BadgeSummaryCard } from '@/components/badges/badge-summary-card';

const GAP = 10;

export default function BadgesScreen() {
  const t = useTheme();
  const tr = useT();
  const features = useFeatures();
  const query = useBadges();
  const { data } = query;
  const [open, setOpen] = useState<Badge | null>(null);
  const [width, setWidth] = useState(0);
  const cols = width >= 560 ? 3 : 2;
  const tile = width ? Math.floor((width - GAP * (cols - 1)) / cols) : 0;

  return (
    <Screen tabs={false}>
      <BackButton />
      <Display size={30}>{tr('badges.title')}</Display>
      {features.loaded && !features.badges ? (
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>{tr('badges.unavailable')}</Text>
      ) : (
        <QueryState query={query} />
      )}
      {data && (
        <>
          <BadgeSummaryCard earned={data.earned} total={data.total} />
          <View onLayout={(e) => setWidth(e.nativeEvent.layout.width)} style={{ flexDirection: 'row', flexWrap: 'wrap', gap: GAP }}>
            {tile > 0 &&
              data.badges.map((b, i) => (
                <BadgeTile key={b.key} badge={b} width={tile} index={i} shown={data.title === b.key && !!chipOf(b)} onOpen={setOpen} />
              ))}
          </View>
        </>
      )}
      <BadgeDetail badge={open} shown={!!open && data?.title === open.key} onClose={() => setOpen(null)} />
    </Screen>
  );
}
