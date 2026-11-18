import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Download, Plus, Trash2 } from 'lucide-react-native';
import { Text } from 'react-native';
import Animated from 'react-native-reanimated';
import { IconButton, PackCard, PageHeader } from '@/components/packs/pack-card';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { type BatchAction, confirmBatch, eachOf, SelectableRow, SelectButton, SelectionBar, SelectionHeader, useSelection } from '@/components/ui/selection';
import { showToast } from '@/lib/toast';
import { api } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { packKeys, type PackItem, useMyPacks } from '@/lib/packs';
import { keys, useGroup } from '@/lib/queries';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Display } from '@/components/ui/text';

type Deleted = { deleted: number[]; skipped: { id: number; error: string }[] };

/** Whether a pack of mine can go into a room: public, or still in review (as on its page). */
const installable = (p: PackItem) => p.status === 'public' || p.status === 'review';

/**
 * The packs I published, with their moderation status (in review, removed and why).
 * Several at once (hold one, or the header's button): delete them, or add them to the current room.
 */
export default function MyPacksScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const mine = useMyPacks();
  const packs = mine.data;
  const { data: group } = useGroup();
  const { batchDelete } = useFeatures();
  const sel = useSelection(packs?.map((p) => p.id));
  const picked = packs?.filter((p) => sel.has(p.id)) ?? [];
  const toAdd = picked.filter(installable);
  // A room where only the owner adds questions: not for me to add packs to.
  const mayAdd = !!group && (group.settings.questions_by !== 'owner' || group.my_role === 'owner');

  const remove = useMutation({
    meta: { quiet: true },
    mutationFn: (ids: number[]) => api<Deleted>('/packs/delete', { method: 'POST', json: { ids } }),
    onSuccess: (r, ids) => {
      showToast(
        r.skipped.length
          ? tr('common.select.partial', { done: r.deleted.length, count: ids.length })
          : tr('packs.mineSelected.deleted', { count: r.deleted.length }),
      );
      sel.cancel();
    },
    onError: () => showToast(tr('common.failed')),
    onSettled: () => qc.invalidateQueries({ queryKey: packKeys.all }),
  });
  const askRemove = async () => {
    const ids = sel.selected;
    if (await confirmBatch(tr('packs.mineSelected.delete', { count: ids.length }), tr('common.delete'), tr('packs.mineSelected.deleteBody')))
      remove.mutate(ids);
  };

  // No batch endpoint: one install per pack, like its page does (each skips questions already in the room).
  const add = useMutation({
    meta: { quiet: true },
    mutationFn: (ids: number[]) => eachOf(ids, (id) => api(`/packs/${id}/install`, { method: 'POST' }), 1),
    onSuccess: (done, ids) => {
      showToast(
        done.length < ids.length
          ? tr('common.select.partial', { done: done.length, count: ids.length })
          : tr('packs.mineSelected.added', { count: done.length, room: group?.name ?? '' }),
      );
      if (done.length) sel.cancel();
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: packKeys.all });
      qc.invalidateQueries({ queryKey: keys.queue });
      qc.invalidateQueries({ queryKey: keys.current });
    },
  });

  const actions: BatchAction[] = [
    ...(mayAdd
      ? [
          {
            icon: Download,
            label: tr('packs.mineSelected.addHere'),
            tone: 'acc' as const,
            disabled: !toAdd.length || remove.isPending,
            busy: add.isPending,
            onPress: () => add.mutate(toAdd.map((p) => p.id)),
          },
        ]
      : []),
    ...(batchDelete
      ? [{ icon: Trash2, label: tr('common.delete'), tone: 'danger' as const, disabled: add.isPending, busy: remove.isPending, onPress: askRemove }]
      : []),
  ];
  const selectable = !!packs?.length && actions.length > 0;

  return (
    <Screen tabs={false} footer={sel.on ? <SelectionBar sel={sel} actions={actions} /> : undefined}>
      {sel.on ? (
        <SelectionHeader sel={sel} />
      ) : (
        <PageHeader fallback="/packs">
          {selectable && <SelectButton onPress={() => sel.start()} />}
          <IconButton icon={Plus} label={tr('packs.create')} onPress={() => router.push('/pack/edit')} />
        </PageHeader>
      )}
      <Display size={30}>{tr('packs.mine')}</Display>
      <QueryState query={mine} />
      {packs?.length === 0 && (
        <>
          <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut, textAlign: 'center', marginTop: 24 }}>{tr('packs.mineNone')}</Text>
          <Button onPress={() => router.push('/pack/edit')}>{tr('packs.create')}</Button>
        </>
      )}
      {packs?.map((p, i) => (
        <Animated.View key={p.id} entering={motion.stagger(i)} exiting={motion.exit} layout={motion.layout}>
          <SelectableRow sel={sel} id={p.id} label={p.name}>
            <PackCard pack={p} roomLang={group?.language} withStatus onLongPress={selectable ? () => sel.start(p.id) : undefined} />
          </SelectableRow>
        </Animated.View>
      ))}
    </Screen>
  );
}
