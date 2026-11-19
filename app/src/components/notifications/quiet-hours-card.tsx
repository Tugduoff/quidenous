import { ArrowRight, Moon } from 'lucide-react-native';
import Animated from 'react-native-reanimated';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import type { NotifPrefs } from '@/lib/api/notifications';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { Toggle } from '@/components/ui/toggle';
import { NotifLine } from '@/components/notifications/notif-line';
import { TimeChip } from '@/components/notifications/time-chip';

const QUIET_START = 22 * 60;
const QUIET_END = 8 * 60;

export function QuietHoursCard({
  quietHours,
  onSet,
  onPick,
}: {
  quietHours: { start: number; end: number } | null;
  onSet: (patch: Partial<NotifPrefs>) => void;
  onPick: (which: 'start' | 'end') => void;
}) {
  const t = useTheme();
  const tr = useT();
  const quiet = quietHours != null;
  return (
  <Card style={{ padding: 16, gap: 14 }}>
    <NotifLine
      Icon={Moon}
      on={quiet}
      title={tr('notifications.quiet')}
      right={
        <Toggle
          label=""
          a11y={tr('notifications.quiet')}
          value={quiet}
          onChange={(v) => onSet(v ? { quiet_start: QUIET_START, quiet_end: QUIET_END } : { quiet_start: null, quiet_end: null })}
        />
      }
    />
    {quietHours && (
      <Animated.View entering={motion.enter} exiting={motion.exit} style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <TimeChip label={tr('notifications.start')} value={quietHours.start} onPress={() => onPick('start')} />
        <ArrowRight size={16} color={t.mut} />
        <TimeChip label={tr('notifications.end')} value={quietHours.end} onPress={() => onPick('end')} />
      </Animated.View>
    )}
  </Card>
  );
}
