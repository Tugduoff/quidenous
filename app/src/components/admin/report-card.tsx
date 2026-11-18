import { Ban, Check, Eye, Lightbulb, type LucideIcon, MessageCircle, MessageSquareQuote, Trash2, User, UserX } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { MiniTag } from '@/components/packs/pack-card';
import { useT } from '@/lib/i18n';
import { REASONS } from '@/lib/api/moderation';
import type { ReportAction, ReviewReport } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';

const KIND_ICONS: Record<ReviewReport['kind'], LucideIcon> = {
  message: MessageCircle,
  member: User,
  question: MessageSquareQuote,
  suggestion: Lightbulb,
};

/** One reported thing: what it says, who and where, why (reasons and details), and what to do with it. */
export function ReportCard({ report: r, busy, onAct }: { report: ReviewReport; busy: boolean; onAct: (action: ReportAction) => void }) {
  const t = useTheme();
  const tr = useT();
  const Icon = KIND_ICONS[r.kind];
  const reasons = REASONS.filter((x) => r.reasons.includes(x)).map((x) => tr(`moderation.reasons.${x}`));
  const who = [r.person && !r.person.deleted ? r.person.name : null, r.room].filter(Boolean).join(' · ');
  const account = !!r.person && !r.person.deleted;
  return (
    <Card style={{ padding: 16, gap: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <Icon size={16} color={t.mut} />
        <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 13, color: t.tagInk }}>{tr('admin.reports', { count: r.count })}</Text>
        {r.hidden && <MiniTag label={tr('admin.hidden')} />}
        {r.gone && <MiniTag label={tr('admin.gone')} />}
        {r.person?.suspended && <MiniTag label={tr('admin.suspended')} tone="hot" />}
      </View>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 16, lineHeight: 22, color: r.gone ? t.mut : t.txt }}>{r.content}</Text>
      {!!who && (
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {who}
        </Text>
      )}
      <Text style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.txt }}>
        {[reasons.join(', '), ...r.details].filter(Boolean).join(' · ')}
      </Text>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8, marginTop: 4 }}>
        <Pill icon={r.hidden ? Eye : Check} label={tr(r.hidden ? 'admin.restore' : 'admin.dismiss')} busy={busy} onPress={() => onAct('dismiss')} />
        {r.kind !== 'member' && !r.gone && <Pill icon={Trash2} label={tr('admin.delete')} danger busy={busy} onPress={() => onAct('delete')} />}
        {account && !r.person?.suspended && <Pill icon={Ban} label={tr('admin.suspend')} danger busy={busy} onPress={() => onAct('suspend')} />}
        {account && <Pill icon={UserX} label={tr('admin.erase')} danger busy={busy} onPress={() => onAct('erase')} />}
      </View>
    </Card>
  );
}

function Pill({ icon: Icon, label, onPress, busy, danger }: { icon: LucideIcon; label: string; onPress: () => void; busy: boolean; danger?: boolean }) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      disabled={busy}
      onPress={onPress}
      style={{
        height: 38,
        paddingHorizontal: 14,
        borderRadius: t.radius.pill,
        flexDirection: 'row',
        gap: 6,
        alignItems: 'center',
        backgroundColor: t.card2,
      }}>
      <Icon size={15} color={danger ? t.tagInk : t.txt} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{label}</Text>
    </PressableScale>
  );
}
