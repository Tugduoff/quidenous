import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Check, X } from 'lucide-react-native';
import { useState } from 'react';
import { Text, TextInput } from 'react-native';
import Animated from 'react-native-reanimated';
import { PageHeader } from '@/components/packs/pack-card';
import { Screen } from '@/components/ui/screen';
import { confirmBatch, eachOf, type Selection, SelectableRow, SelectionBar, useSelection } from '@/components/ui/selection';
import { showToast } from '@/lib/toast';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { packKeys, type ReportAction, type Review, type ReviewReport, removePack, rejectSuggestion, approvePack, approveSuggestion, closeReport } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { Display } from '@/components/ui/text';
import { askConfirm } from '@/lib/confirm';
import { ConfirmModal } from '@/components/ui/dialog';
import { Hold, SectionTitle } from '@/components/admin/selectable-section';
import { ReportCard } from '@/components/admin/report-card';
import { SuggestionCard } from '@/components/admin/suggestion-card';
import { PackReviewCard } from '@/components/admin/pack-review-card';
import { WrappedPreviewLink } from '@/components/admin/wrapped-preview-link';

const reportKey = (r: ReviewReport) => `${r.kind}:${r.id}`;

type Batch = 'dismiss' | 'approvePacks' | 'removePacks' | 'approveSuggestions' | 'rejectSuggestions';

export function AdminReview({ admin, showWrappedPreview, review }: { admin: boolean | undefined; showWrappedPreview: boolean; review: Review | undefined }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const [removing, setRemoving] = useState<number | 'batch' | null>(null);
  const [reason, setReason] = useState('');
  const reportSel = useSelection(review?.reports?.map(reportKey));
  const packSel = useSelection(review?.packs.map((p) => p.id));
  const sugSel = useSelection(review?.suggestions.map((s) => s.id));
  const startIn =
    <K,>(sel: Selection<K>) =>
    (id?: K) => {
      reportSel.cancel();
      packSel.cancel();
      sugSel.cancel();
      sel.start(id);
    };
  const [running, setRunning] = useState<Batch | null>(null);

  const act = useMutation({
    meta: { quiet: true },
    mutationFn: ({ run }: { run: () => Promise<unknown>; drop: { pack?: number; suggestion?: number } }) => run(),
    onSuccess: (_, { drop }) => {
      qc.setQueryData<Review>(
        packKeys.review,
        (r) => r && { ...r, packs: r.packs.filter((p) => p.id !== drop.pack), suggestions: r.suggestions.filter((s) => s.id !== drop.suggestion) },
      );
      if (drop.pack) qc.invalidateQueries({ queryKey: packKeys.all });
      setRemoving(null);
      setReason('');
    },
  });
  const close = useMutation({
    meta: { quiet: true },
    mutationFn: (v: { report: ReviewReport; action: ReportAction }) => closeReport(v.report, v.action),
    onSuccess: (_, { report }) =>
      qc.setQueryData<Review>(packKeys.review, (r) => r && { ...r, reports: r.reports?.filter((x) => x.kind !== report.kind || x.id !== report.id) }),
  });
  const decide = async (report: ReviewReport, action: ReportAction) => {
    if (action === 'suspend' && !(await askConfirm(tr('admin.suspendTitle', { name: report.person?.name ?? '' }), tr('admin.suspendBody'), tr('admin.suspend'))))
      return;
    if (action === 'erase' && !(await askConfirm(tr('admin.eraseTitle', { name: report.person?.name ?? '' }), tr('admin.eraseBody'), tr('admin.erase'))))
      return;
    close.mutate({ report, action });
  };
  const runBatch = async <K,>(what: Batch, sel: Selection<K>, one: (id: K) => Promise<unknown>, drop: (r: Review, done: Set<K>) => Review) => {
    const ids = sel.selected;
    setRunning(what);
    const done = await eachOf(ids, one);
    setRunning(null);
    if (done.length) qc.setQueryData<Review>(packKeys.review, (r) => r && drop(r, new Set(done)));
    if (done.length && (what === 'approvePacks' || what === 'removePacks')) qc.invalidateQueries({ queryKey: packKeys.all });
    if (done.length < ids.length) {
      showToast(tr('common.select.partial', { done: done.length, count: ids.length }));
      qc.invalidateQueries({ queryKey: packKeys.review });
    } else sel.cancel();
  };
  const dropPacks = (r: Review, done: Set<number>): Review => ({ ...r, packs: r.packs.filter((p) => !done.has(p.id)) });
  const dropSuggestions = (r: Review, done: Set<number>): Review => ({ ...r, suggestions: r.suggestions.filter((s) => !done.has(s.id)) });
  const dismissPicked = () => {
    const byKey = new Map(review?.reports?.map((r) => [reportKey(r), r]));
    runBatch(
      'dismiss',
      reportSel,
      (key) => {
        const r = byKey.get(key);
        return r ? closeReport(r, 'dismiss') : Promise.reject(new Error('gone'));
      },
      (r, done) => ({ ...r, reports: r.reports?.filter((x) => !done.has(reportKey(x))) }),
    );
  };
  const removePicked = async () => {
    const why = reason.trim();
    await runBatch('removePacks', packSel, (id) => removePack(id, why), dropPacks);
    setRemoving(null);
    setReason('');
  };
  const rejectPicked = async () => {
    if (!(await confirmBatch(tr('admin.selected.rejectTitle', { count: sugSel.count }), tr('admin.reject')))) return;
    runBatch('rejectSuggestions', sugSel, (id) => rejectSuggestion(id), dropSuggestions);
  };
  const busy = act.isPending || close.isPending || !!running;
  const footer = reportSel.on ? (
    <SelectionBar
      sel={reportSel}
      actions={[{ icon: Check, label: tr('admin.dismiss'), busy: running === 'dismiss', disabled: busy, onPress: dismissPicked }]}
    />
  ) : packSel.on ? (
    <SelectionBar
      sel={packSel}
      actions={[
        { icon: X, label: tr('admin.remove'), tone: 'danger', busy: running === 'removePacks', disabled: busy, onPress: () => setRemoving('batch') },
        {
          icon: Check,
          label: tr('admin.approve'),
          tone: 'acc',
          busy: running === 'approvePacks',
          disabled: busy,
          onPress: () => runBatch('approvePacks', packSel, (id) => approvePack(id), dropPacks),
        },
      ]}
    />
  ) : sugSel.on ? (
    <SelectionBar
      sel={sugSel}
      actions={[
        { icon: X, label: tr('admin.reject'), tone: 'danger', busy: running === 'rejectSuggestions', disabled: busy, onPress: rejectPicked },
        {
          icon: Check,
          label: tr('admin.approve'),
          tone: 'acc',
          busy: running === 'approveSuggestions',
          disabled: busy,
          onPress: () => runBatch('approveSuggestions', sugSel, (id) => approveSuggestion(id), dropSuggestions),
        },
      ]}
    />
  ) : undefined;
  const empty = review && !review.packs.length && !review.suggestions.length && !review.reports?.length;

  return (
    <Screen tabs={false} footer={footer}>
      <PageHeader fallback="/packs" />
      <Display size={30}>{tr('admin.title')}</Display>
      {showWrappedPreview && <WrappedPreviewLink />}
      {(empty || admin === false) && (
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut, textAlign: 'center', marginTop: 24 }}>{tr('admin.empty')}</Text>
      )}
      {(act.isError || close.isError) && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tagInk }}>{tr('common.failed')}</Text>}

      {!!review?.reports?.length && <SectionTitle title={tr('admin.reported')} sel={reportSel} onStart={startIn(reportSel)} />}
      {review?.reports?.map((r, i) => (
        <Animated.View key={`r${r.kind}${r.id}`} entering={motion.stagger(i)} exiting={motion.exit} layout={motion.layout}>
          <SelectableRow sel={reportSel} id={reportKey(r)} label={r.content}>
            <Hold onHold={() => startIn(reportSel)(reportKey(r))}>
              <ReportCard report={r} busy={close.isPending || !!running} onAct={(action) => decide(r, action)} />
            </Hold>
          </SelectableRow>
        </Animated.View>
      ))}

      {!!review?.packs.length && <SectionTitle title={tr('admin.packs')} sel={packSel} onStart={startIn(packSel)} />}
      {review?.packs.map((p, i) => (
        <Animated.View key={`p${p.id}`} entering={motion.stagger(i)} exiting={motion.exit} layout={motion.layout}>
          <SelectableRow sel={packSel} id={p.id} label={p.name}>
            <Hold onHold={() => startIn(packSel)(p.id)}>
              <PackReviewCard
                pack={p}
                busy={act.isPending || !!running}
                onHold={() => startIn(packSel)(p.id)}
                onRemove={() => setRemoving(p.id)}
                onApprove={() => act.mutate({ run: () => approvePack(p.id), drop: { pack: p.id } })}
              />
            </Hold>
          </SelectableRow>
        </Animated.View>
      ))}

      {!!review?.suggestions.length && <SectionTitle title={tr('admin.suggestions')} sel={sugSel} onStart={startIn(sugSel)} />}
      {review?.suggestions.map((s, i) => (
        <Animated.View key={`s${s.id}`} entering={motion.stagger(i)} exiting={motion.exit} layout={motion.layout}>
          <SelectableRow sel={sugSel} id={s.id} label={s.text}>
            <Hold onHold={() => startIn(sugSel)(s.id)}>
              <SuggestionCard
                text={s.text}
                rank={s.kind === 'rank'}
                meta={tr('admin.votes', { up: s.up, down: s.down, author: s.author })}
                lang={s.lang}
                busy={act.isPending || !!running}
                onReject={() => act.mutate({ run: () => rejectSuggestion(s.id), drop: { suggestion: s.id } })}
                onApprove={(tags) => act.mutate({ run: () => approveSuggestion(s.id, tags), drop: { suggestion: s.id } })}
              />
            </Hold>
          </SelectableRow>
        </Animated.View>
      ))}

      <ConfirmModal
        visible={removing != null}
        title={removing === 'batch' ? tr('admin.selected.removeTitle', { count: packSel.count }) : tr('admin.removeTitle')}
        confirmLabel={tr('admin.remove')}
        busy={act.isPending || running === 'removePacks'}
        confirmDisabled={!reason.trim()}
        onCancel={() => setRemoving(null)}
        onConfirm={() =>
          removing === 'batch'
            ? removePicked()
            : removing != null && act.mutate({ run: () => removePack(removing, reason.trim()), drop: { pack: removing } })
        }>
        <TextInput
          accessibilityLabel={tr('admin.reason')}
          placeholder={tr('admin.reason')}
          placeholderTextColor={t.mut}
          value={reason}
          onChangeText={setReason}
          maxLength={200}
          autoFocus
          style={{
            height: 44,
            paddingHorizontal: 14,
            borderRadius: 14,
            backgroundColor: t.card2,
            fontFamily: t.font.regular,
            fontSize: 16,
            color: t.txt,
          }}
        />
      </ConfirmModal>
    </Screen>
  );
}
