import { Screen } from '@/components/ui/screen';
import { useT } from '@/lib/i18n';
import { Display } from '@/components/ui/text';
import { BackButton } from '@/components/ui/back-button';
import { NotificationSettings } from '@/components/notifications/notification-settings';

export default function NotificationsScreen() {
  const tr = useT();
  return (
    <Screen tabs={false}>
      <BackButton />
      <Display size={30}>{tr('notifications.title')}</Display>
      <NotificationSettings />
    </Screen>
  );
}
