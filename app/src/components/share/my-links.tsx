import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ChevronRight, Link2, Trash2 } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { showToast } from '@/lib/toast';
import { dateFormat, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import { useFeatures } from '@/lib/api/features';
import { type MyLink, shareKeys, revokeLink, useMyLinks } from '@/lib/api/share';

/** Profile > "Mes liens": my public share links, each deletable. Hidden while there are none. */
export function MyLinksRow() {
  const t = useTheme();
  const tr = useT();
  const { shareLinks } = useFeatures();
  const [open, setOpen] = useState(false);
  const { data: links } = useMyLinks(shareLinks);
  // The last one deleted: nothing left to show.
  const empty = !!links && !links.length;
  if (empty && open) setOpen(false);
  if (!links?.length && !open) return null;
  return (
    <>
      <Pressable accessibilityRole="button" onPress={() => setOpen(true)}>
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
          <Link2 size={20} color={t.txt} />
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('share.links.title')}</Text>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{links?.length ?? 0}</Text>
          <ChevronRight size={20} color={t.mut} />
        </Card>
      </Pressable>
      <Dialog visible={open} onClose={() => setOpen(false)}>
        <DialogTitle title={tr('share.links.title')} />
        <ScrollView style={{ maxHeight: 420 }} contentContainerStyle={{ gap: 6 }}>
          {links?.map((l) => (
            <LinkRow key={l.token} link={l} />
          ))}
        </ScrollView>
      </Dialog>
    </>
  );
}

function LinkRow({ link }: { link: MyLink }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const revoke = useMutation({
    mutationFn: () => revokeLink(link.token),
    onSuccess: () => {
      qc.setQueryData<MyLink[]>(shareKeys.mine, (all) => all?.filter((l) => l.token !== link.token));
      showToast(tr('share.links.revoked'));
    },
  });
  const kind = link.kind === 'stat' ? 'share.links.stat' : link.with_result ? 'share.links.result' : 'share.links.question';
  const until = link.expires_at
    ? tr('share.links.until', { date: dateFormat({ day: 'numeric', month: 'short' }).format(new Date(link.expires_at)) })
    : null;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 8, opacity: revoke.isPending ? 0.5 : 1 }}>
      <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
          {link.title}
        </Text>
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {[tr(kind), link.room, until].filter(Boolean).join(' · ')}
        </Text>
      </View>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('share.links.revoke')}
        disabled={revoke.isPending}
        onPress={() => revoke.mutate()}
        hitSlop={8}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
        <Trash2 size={18} color={t.tagInk} />
      </Pressable>
    </View>
  );
}
