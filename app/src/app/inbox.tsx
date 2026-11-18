import { useState } from 'react';
import { Text } from 'react-native';
import { InboxRow } from '@/components/inbox/inbox-row';
import { BackButton } from '@/components/ui/back-button';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { Display } from '@/components/ui/text';
import { useGroup, useMembers } from '@/lib/api/group';
import { useInbox } from '@/lib/api/inbox';
import { useMarkInboxRead } from '@/lib/hooks/use-inbox';
import { useOpenInboxItem } from '@/lib/hooks/use-open-inbox-item';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

export default function InboxScreen() {
  const t = useTheme();
  const tr = useT();
  const inbox = useInbox();
  const { data } = inbox;
  const { byId } = useMembers();
  const { data: group } = useGroup();
  const session = useSession();
  const room = group?.id ?? session.room;
  const open = useOpenInboxItem(room);
  const unread = data?.unread ?? 0;
  const [fresh, setFresh] = useState<Set<number> | null>(null);
  if (data && !fresh) setFresh(new Set(data.items.filter((i) => !i.read).map((i) => i.id)));
  useMarkInboxRead(unread);

  return (
    <Screen tabs={false}>
      <BackButton />
      <Display size={30}>{tr('inbox.title')}</Display>
      <QueryState query={inbox} />
      {data && data.items.length === 0 && <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>{tr('inbox.empty')}</Text>}
      {data?.items.map((item) => {
        const isNew = !item.read || !!fresh?.has(item.id);
        const here = item.room_id == null || item.room_id === room;
        const actor = item.actor_id != null && here ? byId.get(item.actor_id) : undefined;
        return <InboxRow key={item.id} item={item} actor={actor} isNew={isNew} onOpen={() => open(item)} />;
      })}
    </Screen>
  );
}
