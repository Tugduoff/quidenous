import { Text, View } from 'react-native';
import { Caps } from '@/components/stats/common';
import { TitleChip } from '@/components/badges/title-chip';
import { BadgeProgress } from '@/components/badges/badge-progress';
import { useMyTitle } from '@/lib/api/badges';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';

export function BadgeSummaryCard({ earned, total }: { earned: number; total: number }) {
  const t = useTheme();
  const tr = useT();
  const title = useMyTitle();
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      <View style={{ gap: 8 }}>
        <Caps size={11}>{tr('badges.shown')}</Caps>
        {title ? (
          <TitleChip chip={title} size="md" />
        ) : (
          <View style={{ gap: 2 }}>
            <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>{tr('badges.noTitle')}</Text>
            <Text style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>{tr('badges.noTitleHint')}</Text>
          </View>
        )}
      </View>
      <View style={{ gap: 6 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>
          {tr('badges.count', { earned: earned, total: total })}
        </Text>
        <BadgeProgress value={earned} goal={total} height={6} />
      </View>
    </Card>
  );
}
