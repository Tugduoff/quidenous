import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Ban, CalendarHeart, Check, ChevronRight, Eye, Lightbulb, type LucideIcon, MessageCircle, MessageSquareQuote, Trash2, User, UserX, X } from 'lucide-react-native';
import { type ReactNode, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { MiniTag, PageHeader } from '@/components/packs/pack-card';
import { Screen } from '@/components/ui/screen';
import { confirmBatch, eachOf, type Selection, SelectableRow, SelectButton, SelectionBar, SelectionHeader, useSelection } from '@/components/ui/selection';
import { showToast } from '@/lib/toast';
import { dateFormat, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { REASONS } from '@/lib/api/moderation';
import { type PackTag, packKeys, type ReportAction, REPORT_REASONS, type ReportReason, type Review, type ReviewReport, useCommunityMe, useReview, removePack, rejectSuggestion, approvePack, approveSuggestion, closeReport } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';
import { Display, QuestionText } from '@/components/ui/text';
import { askConfirm } from '@/lib/confirm';
import { ConfirmModal } from '@/components/ui/dialog';
import { useFeatures } from '@/lib/api/features';
import { TAG_ICONS } from '@/components/packs/tag-icons';

/** The built-in deck's tones a suggestion can join (none picked: the server files it under `potes`). */
const DECK_TAGS: PackTag[] = ['potes', 'pimente', 'boulot', 'famille'];
const isReason = (r: string | null): r is ReportReason => !!r && (REPORT_REASONS as string[]).includes(r);
/** A report's id in its list (a message and a member can share a number). */
const reportKey = (r: ReviewReport) => `${r.kind}:${r.id}`;

type Batch = 'dismiss' | 'approvePacks' | 'removePacks' | 'approveSuggestions' | 'rejectSuggestions';

/** Admins: reported messages, members, questions and suggestions; packs sent to review by reports (approve or remove with a reason), suggestions voted up (approve or reject). */
export default function AdminScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { data: me } = useCommunityMe();
  const features = useFeatures();
  const { data: review } = useReview(!!me?.admin);
  // A pack, or the selected ones ('batch'), waiting for a reason to be removed.
  const [removing, setRemoving] = useState<number | 'batch' | null>(null);
  const [reason, setReason] = useState('');
  // Several items of one list at a time (hold a card, or the section's button); starting one list leaves the others.
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
    // Acting on the account: asked first (erasing can't be undone).
    if (action === 'suspend' && !(await askConfirm(tr('admin.suspendTitle', { name: report.person?.name ?? '' }), tr('admin.suspendBody'), tr('admin.suspend'))))
      return;
    if (action === 'erase' && !(await askConfirm(tr('admin.eraseTitle', { name: report.person?.name ?? '' }), tr('admin.eraseBody'), tr('admin.erase'))))
      return;
    close.mutate({ report, action });
  };
  /**
   * No batch endpoint: the single actions' calls on the selected ones, 3 at a time. The done ones leave the list
   * (as after a single action); the failed ones stay ticked, to try again.
   */
  const runBatch = async <K,>(what: Batch, sel: Selection<K>, one: (id: K) => Promise<unknown>, drop: (r: Review, done: Set<K>) => Review) => {
    const ids = sel.selected;
    setRunning(what);
    const done = await eachOf(ids, one);
    setRunning(null);
    if (done.length) qc.setQueryData<Review>(packKeys.review, (r) => r && drop(r, new Set(done)));
    if (done.length && (what === 'approvePacks' || what === 'removePacks')) qc.invalidateQueries({ queryKey: packKeys.all });
    if (done.length < ids.length) {
      showToast(tr('common.select.partial', { done: done.length, count: ids.length }));
      // Some may have been handled by someone else meanwhile: the list as it is now.
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
          // Without deck tones: the server files them under `potes`.
          onPress: () => runBatch('approveSuggestions', sugSel, (id) => approveSuggestion(id), dropSuggestions),
        },
      ]}
    />
  ) : undefined;
  const empty = review && !review.packs.length && !review.suggestions.length && !review.reports?.length;
  const day = dateFormat({ day: 'numeric', month: 'short' });

  return (
    <Screen tabs={false} footer={footer}>
      <PageHeader fallback="/packs" />
      <Display size={30}>{tr('admin.title')}</Display>
      {me?.admin && features.wrapped && (
        <PressableScale
          scale={0.98}
          accessibilityRole="button"
          accessibilityLabel={`${tr('wrapped.adminPreview')}. ${tr('wrapped.adminPreviewLine')}`}
          onPress={() => router.push('/wrapped?preview=1')}>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            <CalendarHeart size={20} color={t.txt} />
            <View style={{ flex: 1, gap: 2 }}>
              <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('wrapped.adminPreview')}</Text>
              <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('wrapped.adminPreviewLine')}</Text>
            </View>
            <ChevronRight size={20} color={t.mut} />
          </Card>
        </PressableScale>
      )}
      {(empty || me?.admin === false) && (
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
              <Card style={{ padding: 16, gap: 8 }}>
                <PressableScale
                  scale={0.98}
                  accessibilityRole="button"
                  onPress={() => router.push(`/pack/${p.id}`)}
                  onLongPress={() => startIn(packSel)(p.id)}
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
                <Actions
                  busy={act.isPending || !!running}
                  onReject={() => setRemoving(p.id)}
                  rejectLabel={tr('admin.remove')}
                  onApprove={() => act.mutate({ run: () => approvePack(p.id), drop: { pack: p.id } })}
                />
              </Card>
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

/** A section's title and its "Sélectionner"; while selecting in it, the count, all / none and leave. */
function SectionTitle<K>({ title, sel, onStart }: { title: string; sel: Selection<K>; onStart: () => void }) {
  const t = useTheme();
  if (sel.on) return <SelectionHeader sel={sel} />;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 4 }}>
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{title}</Text>
      <SelectButton size={32} onPress={() => onStart()} />
    </View>
  );
}

/** A hold on a card (outside its buttons, which keep their own taps) starts selecting. */
function Hold({ onHold, children }: { onHold: () => void; children: ReactNode }) {
  return (
    <Pressable accessible={false} onLongPress={onHold} delayLongPress={400}>
      {children}
    </Pressable>
  );
}

/** A queued suggestion: the question, its votes, the deck tones to file it under, reject / approve. */
function SuggestionCard({
  text,
  rank,
  meta,
  lang,
  busy,
  onReject,
  onApprove,
}: {
  text: string;
  rank: boolean;
  meta: string;
  lang: string;
  busy: boolean;
  onReject: () => void;
  onApprove: (tags: PackTag[]) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const [tags, setTags] = useState<PackTag[]>([]);
  return (
    <Card style={{ padding: 16, gap: 10 }}>
      <QuestionText text={text} rank={rank} style={{ fontFamily: t.font.semibold, fontSize: 16, lineHeight: 22, color: t.txt }} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {meta}
        </Text>
        <MiniTag label={lang.toUpperCase()} />
      </View>
      <View style={{ flexDirection: 'row', gap: 6 }}>
        {DECK_TAGS.map((id) => {
          const on = tags.includes(id);
          const Icon = TAG_ICONS[id];
          return (
            <PressableScale
              key={id}
              accessibilityRole="button"
              accessibilityLabel={tr(`packs.tags.${id}`)}
              accessibilityState={{ selected: on }}
              onPress={() => setTags((l) => (on ? l.filter((x) => x !== id) : [...l, id]))}
              style={{
                width: 34,
                height: 34,
                borderRadius: 17,
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: on ? t.acc : t.card2,
              }}>
              <Icon size={16} color={on ? t.accTxt : t.mut} />
            </PressableScale>
          );
        })}
      </View>
      <Actions busy={busy} onReject={onReject} rejectLabel={tr('admin.reject')} onApprove={() => onApprove(tags)} />
    </Card>
  );
}

const KIND_ICONS: Record<ReviewReport['kind'], LucideIcon> = {
  message: MessageCircle,
  member: User,
  question: MessageSquareQuote,
  suggestion: Lightbulb,
};

/** One reported thing: what it says, who and where, why (reasons and details), and what to do with it. */
function ReportCard({ report: r, busy, onAct }: { report: ReviewReport; busy: boolean; onAct: (action: ReportAction) => void }) {
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

/** Reject (or remove) and approve, side by side. */
function Actions({ busy, onReject, rejectLabel, onApprove }: { busy: boolean; onReject: () => void; rejectLabel: string; onApprove: () => void }) {
  const t = useTheme();
  const tr = useT();
  const pill = {
    flex: 1,
    height: 42,
    borderRadius: t.radius.pill,
    flexDirection: 'row' as const,
    gap: 6,
    alignItems: 'center' as const,
    justifyContent: 'center' as const,
  };
  return (
    <View style={{ flexDirection: 'row', gap: 10, marginTop: 4 }}>
      <PressableScale accessibilityRole="button" disabled={busy} onPress={onReject} style={[pill, { backgroundColor: t.card2 }]}>
        <X size={16} color={t.tagInk} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{rejectLabel}</Text>
      </PressableScale>
      <PressableScale accessibilityRole="button" disabled={busy} onPress={onApprove} style={[pill, { backgroundColor: t.acc }]}>
        <Check size={16} color={t.accTxt} />
        <Text style={{ fontFamily: t.font.bold, fontSize: 14, color: t.accTxt }}>{tr('admin.approve')}</Text>
      </PressableScale>
    </View>
  );
}
