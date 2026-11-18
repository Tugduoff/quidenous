import { AtSign, BellRing, Crown, Flame, type LucideIcon, MessageCircle, MessageCircleQuestion, Sparkles, UserPlus } from 'lucide-react-native';
import { type TKey, useT } from '@/lib/i18n';
import type { NotifKind } from '@/lib/api/notifications';
import { Toggle } from '@/components/ui/toggle';
import { Section } from '@/components/ui/section';
import { NotifLine } from '@/components/notifications/notif-line';

const KINDS: { kind: NotifKind; label: TKey; Icon: LucideIcon }[] = [
  { kind: 'question', label: 'notifications.kinds.question', Icon: Sparkles },
  { kind: 'result', label: 'notifications.kinds.result', Icon: Crown },
  { kind: 'chat', label: 'notifications.kinds.chat', Icon: MessageCircle },
  { kind: 'mention', label: 'notifications.kinds.mention', Icon: AtSign },
  { kind: 'nudge', label: 'notifications.kinds.nudge', Icon: BellRing },
  { kind: 'added', label: 'notifications.kinds.added', Icon: UserPlus },
  { kind: 'ask', label: 'notifications.kinds.ask', Icon: MessageCircleQuestion },
  // Only from a server with streaks (docs/STREAKS.md).
  { kind: 'streak', label: 'streaks.notif', Icon: Flame },
];

export function NotifKindsSection({ off, streaks, onOff }: { off: NotifKind[]; streaks: boolean; onOff: (off: NotifKind[]) => void }) {
  const tr = useT();
  return (
    <Section title={tr('notifications.tellMe')}>
      {KINDS.filter((k) => k.kind !== 'streak' || streaks).map(({ kind, label, Icon }) => {
        const on = !off.includes(kind);
        return (
          <NotifLine
            key={kind}
            Icon={Icon}
            on={on}
            title={tr(label)}
            right={
              <Toggle
                label=""
                a11y={tr(label)}
                value={on}
                onChange={(v) => onOff(v ? off.filter((k) => k !== kind) : [...off, kind])}
              />
            }
          />
        );
      })}
    </Section>
  );
}
