import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Flag, Pencil, Share2, Star, Trash2 } from 'lucide-react-native';
import { useState } from 'react';
import { Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { IconButton, PageHeader } from '@/components/packs/pack-card';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { ApiError, isNotFound } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { motion, usePop } from '@/lib/motion';
import { packError, packKeys, type PackDetail, useCommunityMe, usePackDetail, installPack, likePack, featurePack, deletePack } from '@/lib/api/packs';
import { useSession } from '@/lib/session';
import { shareText } from '@/lib/device/share';
import { useTheme } from '@/lib/theme';
import { askConfirm } from '@/lib/confirm';
import { useGroup } from '@/lib/api/group';
import { useRooms } from '@/lib/api/rooms';
import { roundKeys } from '@/lib/api/rounds';
import { questionKeys } from '@/lib/api/questions';
import { packLink } from '@/lib/links';
import { PackReportDialog } from '@/components/packs/pack-report-dialog';
import { PackInstallDialog } from '@/components/packs/pack-install-dialog';
import { PackQuestionPreview } from '@/components/packs/pack-question-preview';
import { PackSummary } from '@/components/packs/pack-summary';
import { PackStatsRow } from '@/components/packs/pack-stats-row';
import { PackInstallButton } from '@/components/packs/pack-install-button';

export function PackView({ id }: { id: number }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const detail = usePackDetail(id);
  const pack = detail.data;
  const { data: group } = useGroup();
  const { data: rooms } = useRooms();
  const { data: me } = useCommunityMe();
  const { room } = useSession();
  const here = room ?? group?.id ?? null;
  const several = (rooms?.length ?? 0) > 1;
  const [note, setNote] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);
  const [reporting, setReporting] = useState(false);
  // Rooms it went into from this page (the server only tells about the current room).
  const [installedIn, setInstalledIn] = useState<number[]>([]);
  const isIn = (roomId: number | null) => roomId != null && (installedIn.includes(roomId) || (roomId === here && !!pack?.installed));

  const patch = (p: Partial<PackDetail>) => qc.setQueryData<PackDetail>(packKeys.detail(id), (old) => old && { ...old, ...p });
  const refreshLists = () => qc.invalidateQueries({ queryKey: packKeys.all, predicate: (q) => q.queryKey[1] !== 'detail' });

  const install = useMutation({
    meta: { quiet: true },
    mutationFn: (roomId: number) => installPack(id, roomId),
    onSuccess: (r, roomId) => {
      const name = rooms?.find((x) => x.id === roomId)?.name ?? group?.name ?? '';
      setNote(
        r.skipped
          ? tr('packs.installedSkipped', { added: r.added, room: name, skipped: r.skipped })
          : tr('packs.installed', { added: r.added, room: name }),
      );
      setInstalledIn((l) => [...l, roomId]);
      if (roomId === here) {
        patch({ installed: true });
        qc.invalidateQueries({ queryKey: questionKeys.queue });
        qc.invalidateQueries({ queryKey: roundKeys.current });
      }
      // Its install count (once per room) comes back from the server.
      qc.invalidateQueries({ queryKey: packKeys.all });
    },
    onError: (e) => setNote(tr(e instanceof ApiError ? packError(e.code) : 'common.failed')),
  });
  const like = useMutation({
    mutationFn: (on: boolean) => likePack(id, on),
    onMutate: (on) => pack && patch({ liked: on, likes: Math.max(0, pack.likes + (on ? 1 : -1)) }),
    onSuccess: (r) => patch(r),
    onError: () => qc.invalidateQueries({ queryKey: packKeys.detail(id) }),
    onSettled: refreshLists,
  });
  const feature = useMutation({
    meta: { quiet: true },
    mutationFn: (featured: boolean) => featurePack(id, featured),
    onSuccess: (r) => {
      patch(r);
      refreshLists();
    },
    onError: () => setNote(tr('common.failed')),
  });
  const remove = useMutation({
    meta: { quiet: true },
    mutationFn: () => deletePack(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: packKeys.all });
      router.back();
    },
    onError: () => setNote(tr('common.failed')),
  });
  const pop = usePop(pack?.liked, 1.25, !!pack?.liked);

  if (!pack)
    return (
      <Screen tabs={false}>
        <PageHeader fallback="/packs" />
        {isNotFound(detail.error) ? (
          <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>{tr('packs.notFound')}</Text>
        ) : (
          <QueryState query={detail} />
        )}
      </Screen>
    );

  const listed = pack.status === 'public';
  const installable = listed || (pack.mine && pack.status === 'review');
  const installedHere = isIn(here);
  const share = async () => {
    const r = await shareText(tr('packs.shareMessage', { name: pack.name, link: packLink(id) }));
    if (r === 'copied') setNote(tr('packs.linkCopied'));
  };
  const askRemove = async () => {
    if (await askConfirm(tr('packs.remove'), undefined, tr('packs.removeAction'))) remove.mutate();
  };

  return (
    <Screen
      tabs={false}
      footer={
        installable && (
          <PackInstallButton
            disabled={install.isPending || here == null}
            several={several}
            installedHere={installedHere}
            onPress={() => (several ? setPicking(true) : here != null && install.mutate(here))}
          />
        )
      }>
      <PageHeader fallback="/packs">
        {me?.admin && listed && (
          <IconButton
            icon={Star}
            label={tr(pack.featured ? 'packs.unfeature' : 'packs.feature')}
            on={pack.featured}
            disabled={feature.isPending}
            onPress={() => feature.mutate(!pack.featured)}
          />
        )}
        {listed && <IconButton icon={Share2} label={tr('packs.share')} onPress={share} />}
        {pack.mine && pack.status !== 'removed' && (
          <IconButton icon={Pencil} label={tr('packs.edit')} onPress={() => router.push({ pathname: '/pack/edit', params: { id: String(id) } })} />
        )}
        {pack.mine ? (
          <IconButton icon={Trash2} label={tr('packs.removeAction')} disabled={remove.isPending} onPress={askRemove} />
        ) : (
          <IconButton icon={Flag} label={tr('packs.report')} onPress={() => setReporting(true)} />
        )}
      </PageHeader>

      <PackSummary pack={pack} listed={listed} roomLanguage={group?.language} />

      <PackStatsRow pack={pack} installedHere={installedHere} pop={pop} onLike={(on) => like.mutate(on)} />

      {note && (
        <Animated.Text entering={motion.enter} style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>
          {note}
        </Animated.Text>
      )}

      <PackQuestionPreview questions={pack.questions} />

      <PackInstallDialog
        visible={picking}
        onClose={() => setPicking(false)}
        rooms={rooms}
        isIn={isIn}
        busy={install.isPending}
        onPick={(roomId) => install.mutate(roomId)}
      />

      <PackReportDialog
        visible={reporting}
        onClose={() => setReporting(false)}
        onSent={() => {
          setReporting(false);
          setNote(tr('packs.reported'));
        }}
        id={id}
      />
    </Screen>
  );
}
