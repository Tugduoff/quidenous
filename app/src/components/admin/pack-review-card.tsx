import { router } from 'expo-router';
import { Text, View } from 'react-native';
import { MiniTag } from '@/components/packs/pack-card';
import { dateFormat, useT } from '@/lib/i18n';
import { REPORT_REASONS, type ReportReason, type Review } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';
import { ReviewActions } from '@/components/admin/review-actions';

const isReason = (r: string | null): r is ReportReason => !!r && (REPORT_REASONS as string[]).includes(r);

export function PackReviewCard({
  pack: p,
  busy,
  onHold,
  onRemove,
  onApprove,
}: {
  pack: Review['packs'][number];
  busy: boolean;
  onHold: () => void;
  onRemove: () => void;
  onApprove: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  const day = dateFormat({ day: 'numeric', month: 'short' });
  return (
    <Card style={{ padding: 16, gap: 8 }}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        onPress={() => router.push(`/pack/${p.id}`)}
        onLongPress={onHold}
        delayLongPress={400}
        style={{ gap: 2 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
            {p.name}
          </Text>
          {p.spicy && <MiniTag label={tr('packs.spicy')} tone="hot" />}
        </View>
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {tr('packs.byline', { count: p.count, author: p.author })}
        </Text>
      </PressableScale>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.tagInk }}>{tr('admin.reports', { count: p.reports.length })}</Text>
      {p.reports.map((r, j) => (
        <Text key={j} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.txt }}>
          <Text style={{ fontFamily: t.font.semibold }}>{isReason(r.reason) ? tr(`packs.reasons.${r.reason}`) : '—'}</Text>
          {r.details ? ` · ${r.details}` : ''}
          <Text style={{ color: t.mut }}>{` · ${day.format(new Date(r.created_at))}`}</Text>
        </Text>
      ))}
      <ReviewActions
        busy={busy}
        onReject={onRemove}
        rejectLabel={tr('admin.remove')}
        onApprove={onApprove}
      />
    </Card>
  );
}
