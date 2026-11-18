import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { ArrowDownToLine, ArrowUpToLine, Cable, Check, FileUp, Lightbulb, ListX, Lock, MessageCircleQuestion, Package, Plus, Shuffle, Trash2, Trophy, X } from 'lucide-react-native';
import { type ComponentRef, useEffect, useRef, useState } from 'react';
import { Platform, Pressable, type ScrollView, Text, TextInput, View } from 'react-native';
import Animated, { LayoutAnimationConfig } from 'react-native-reanimated';
import { AnonymousMark, AnonymousToggle } from '@/components/vote/anonymous';
import { ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { ReorderList, useScrollTracker } from '@/components/vote/reorder-list';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { confirmBatch, SelectableRow, SelectButton, SelectionBar, SelectionHeader, useSelection } from '@/components/ui/selection';
import { TemplateLink } from '@/components/packs/template-link';
import { Tip } from '@/components/ui/tip';
import { ApiError } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { ITEM_MAX, ITEMS_MAX, ITEMS_MIN, itemsOk, splitTag, type DraftLine } from '@/lib/utils/import-draft';
import { motion, useArrivals } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { tidy } from '@/lib/utils/text';
import { useTheme } from '@/lib/theme';
import { sendOnEnter } from '@/components/ui/send-on-enter';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { Display, QuestionText } from '@/components/ui/text';
import { Toggle } from '@/components/ui/toggle';
import { Segmented } from '@/components/ui/segmented';
import { askConfirm } from '@/lib/confirm';
import { ConfirmModal } from '@/components/ui/dialog';
import { type QuestionKind, roundKeys } from '@/lib/api/rounds';
import { type Queue, useQueue, questionKeys, deleteQuestion, addQuestion, importQuestions, shuffleQueue, emptyQueue, reorderQueue, deleteQuestions } from '@/lib/api/questions';
import { useGroup, useMembers } from '@/lib/api/group';
import { useFeatures } from '@/lib/api/features';
import { pickQuestionFile, takeHandedDraft } from '@/lib/device/files';

const MIN = 8;

export default function QuestionsScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const queue = useQueue();
  const { data } = queue;
  // The queue as it first loads just shows; questions added afterwards fade in.
  const isNew = useArrivals(data?.custom.map((q) => q.id));
  const { data: group } = useGroup();
  const { byId } = useMembers();
  const [text, setText] = useState('');
  const [rank, setRank] = useState(false);
  // A choose question ("Choix"): the question and its 2 to 4 items.
  const [choose, setChoose] = useState(false);
  const [items, setItems] = useState<string[]>(['', '']);
  // A « Qui a dit ça ? » question: everyone writes an answer, then guesses who wrote what (docs/SAID.md).
  const [said, setSaid] = useState(false);
  // Anonymous votes on it (the room's rule is "per question"); `[anonyme]` typed by hand works too.
  const [anon, setAnon] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);
  const { me } = useSession();
  const features = useFeatures();
  const { community, reorder: canReorder, clearQueue } = features;
  const [toDelete, setToDelete] = useState<{ id: number; text: string } | null>(null);
  // Long-press on someone else's question: report it.
  const [toReport, setToReport] = useState<ModTarget | null>(null);
  const remove = useMutation({
    mutationFn: (id: number) => deleteQuestion(id),
    onSettled: () => {
      setToDelete(null);
      qc.invalidateQueries({ queryKey: questionKeys.queue });
    },
  });
  // Typing `[classement] …` (or `[rank] …`, `[choix] … | A | B`) by hand works too.
  const typed = splitTag(text.trim());
  // A server without choose questions can't take a typed `[choix]` one either.
  const typedChoose = typed.kind === 'choose' && features.choose;
  const kind: QuestionKind = said && features.said ? 'said' : choose || typedChoose ? 'choose' : rank || typed.rank ? 'rank' : 'pick';
  const chosenItems = choose ? items.map((i) => i.trim()) : (typed.items ?? []);
  const perQuestion = features.anonymousVotes && group?.settings.anonymous_votes === 'per_question';
  const anonymous = perQuestion && kind !== 'choose' && kind !== 'said' && (anon || typed.anonymous);
  const sameItems = new Set(chosenItems.filter(Boolean).map((i) => i.toLowerCase())).size < chosenItems.filter(Boolean).length;
  const add = useMutation({
    // A bad list of answers shows under it; any other failure in a toast.
    meta: { quiet: (e) => e instanceof ApiError && e.code === 'bad_items' },
    mutationFn: () =>
      addQuestion(
        kind === 'choose'
          ? { text: typed.text, kind, items: chosenItems }
          : kind === 'said'
            ? { text: typed.text, kind }
            : { text: typed.text, kind, ...(anonymous ? { anonymous } : {}) },
      ),
    onSuccess: () => {
      setText('');
      setRank(false);
      setChoose(false);
      setSaid(false);
      setItems(['', '']);
      setAnon(false);
      setAddError(null);
      qc.invalidateQueries({ queryKey: questionKeys.queue });
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
    onError: (e) => setAddError(e instanceof ApiError && e.code === 'bad_items' ? tr('questions.badItems') : null),
  });
  const ok = typed.text.length >= MIN && (kind !== 'choose' || itemsOk(chosenItems)) && (typed.kind !== 'choose' || typedChoose) && !add.isPending;
  const setItem = (i: number, v: string) => setItems((l) => l.map((x, j) => (j === i ? v : x)));
  const [imported, setImported] = useState<string | null>(null);
  // Pick a file, show what's in it, send only what's ticked.
  const [draft, setDraft] = useState<DraftLine[] | null>(null);
  const [mix, setMix] = useState(false);
  const review = (lines: DraftLine[]) => {
    setMix(false);
    setDraft(lines);
  };
  const pick = async () => {
    const lines = await pickQuestionFile();
    if (lines === 'empty') setImported(tr('questions.emptyFile'));
    else if (lines) review(lines);
  };
  // Sent here from the Vote tab's "no more questions": `compose` opens the keyboard, `import` brings the file picked there.
  const params = useLocalSearchParams<{ compose?: string; import?: string }>();
  const composer = useRef<ComponentRef<typeof TextInput>>(null);
  useEffect(() => {
    if (!params.compose && !params.import) return;
    const handed = params.import ? takeHandedDraft() : null;
    if (handed) {
      setMix(false);
      setDraft(handed);
    }
    router.setParams({ compose: undefined, import: undefined });
    if (!params.compose) return;
    // Once the tab has slid in.
    const focus = setTimeout(() => composer.current?.focus(), 350);
    return () => clearTimeout(focus);
  }, [params.compose, params.import]);
  const importFile = useMutation({
    meta: { quiet: true },
    mutationFn: (lines: string[]) =>
      importQuestions(lines),
    onSuccess: (res) => {
      setDraft(null);
      setImported(
        res.skipped ? tr('questions.importedSkipped', { added: res.added, skipped: res.skipped }) : tr('questions.imported', { added: res.added }),
      );
      qc.invalidateQueries({ queryKey: questionKeys.queue });
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
    onError: (e) =>
      setImported(tr(e instanceof ApiError && e.code === 'too_many_questions' ? 'questions.importTooMany' : 'questions.importFailed')),
  });
  const chosen = draft?.filter((l) => l.selected).map((l) => l.line) ?? [];
  // Shuffle the whole upcoming queue (questions added later still go last).
  const shuffle = useMutation({
    meta: { quiet: true },
    mutationFn: () => shuffleQueue(),
    onSuccess: () => {
      setImported(tr('questions.shuffled'));
      qc.invalidateQueries({ queryKey: questionKeys.queue });
    },
    onError: () => setImported(tr('questions.shuffleFailed')),
  });
  // Owner: delete every upcoming question of the room at once (the built-in deck stays).
  const clear = useMutation({
    meta: { quiet: true },
    mutationFn: () => emptyQueue(),
    onSuccess: (res) => setImported(tr('questions.cleared', { count: res?.deleted ?? 0 })),
    onError: () => setImported(tr('questions.clearFailed')),
    onSettled: () => {
      qc.invalidateQueries({ queryKey: questionKeys.queue });
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
  });
  const askClear = async () => {
    const count = data?.custom.length ?? 0;
    if (await askConfirm(tr('questions.clearTitle', { count }), undefined, tr('common.delete'))) clear.mutate();
  };
  // Hold a question and drag it: the new order shows at once, and comes back if the server says no.
  const scroller = useRef<ComponentRef<typeof ScrollView>>(null);
  const tracker = useScrollTracker();
  const [dragging, setDragging] = useState(false);
  const reorder = useMutation({
    meta: { quiet: true },
    mutationFn: (ids: number[]) => reorderQueue(ids),
    // Someone added or removed a question meanwhile: the refetch below shows the real order, no error.
    onError: (e) => !(e instanceof ApiError && e.code === 'stale_queue') && setImported(tr('questions.reorderFailed')),
    onSettled: () => qc.invalidateQueries({ queryKey: questionKeys.queue }),
  });
  const reordered = (ids: number[]) => {
    const before = qc.getQueryData<Queue>(questionKeys.queue);
    qc.cancelQueries({ queryKey: questionKeys.queue });
    if (before) {
      const at = new Map(ids.map((id, i) => [id, i]));
      const custom = [...before.custom].sort((a, b) => (at.get(a.id) ?? Infinity) - (at.get(b.id) ?? Infinity));
      qc.setQueryData<Queue>(questionKeys.queue, { ...before, custom });
    }
    reorder.mutate(ids, { onError: () => before && qc.setQueryData(questionKeys.queue, before) });
  };
  const toggle = (i: number) => setDraft((d) => d && d.map((l, j) => (j === i && !l.problem ? { ...l, selected: !l.selected } : l)));
  const allOn = !!draft && draft.every((l) => l.selected || l.problem);
  const toggleAll = () => setDraft((d) => d && d.map((l) => (l.problem ? l : { ...l, selected: !allOn })));

  // The owner can keep questions to themselves (room setting).
  const locked = group?.settings.questions_by === 'owner' && group.my_role !== 'owner';
  const sortable = canReorder && !locked && (data?.custom.length ?? 0) > 1;
  const othersQuestion = (q: Queue['custom'][number]) => q.author_id != null && q.author_id !== me?.id;
  const canDelete = (q: Queue['custom'][number]) => q.author_id === me?.id || group?.my_role === 'owner';
  // Several at once: "Sélectionner" in the header (a hold already drags or reports), tick, then move or delete them.
  // Without reordering on the server, only the questions I can delete can be ticked.
  const canMove = canReorder && !locked;
  const canDeleteMany = features.batchDelete && !locked;
  const canSelect = canMove || canDeleteMany;
  const selectable = data?.custom.filter((q) => canMove || canDelete(q)).map((q) => q.id);
  const sel = useSelection(selectable);
  const { cancel: leaveSelection } = sel;
  // The room just locked its questions: back to the plain list.
  useEffect(() => {
    if (!canSelect) leaveSelection();
  }, [canSelect, leaveSelection]);
  // The ticked ones first or last, the others keep their order.
  const moveSelected = (first: boolean) => {
    if (!data || !sel.count) return;
    const picked = new Set(sel.selected);
    const rest = data.custom.map((q) => q.id).filter((id) => !picked.has(id));
    const count = sel.count;
    reordered(first ? [...sel.selected, ...rest] : [...rest, ...sel.selected]);
    sel.cancel();
    setImported(tr(first ? 'questions.movedFirst' : 'questions.movedLast', { count }));
  };
  const removeMany = useMutation({
    meta: { quiet: true },
    mutationFn: (ids: number[]) =>
      deleteQuestions(ids),
    onSuccess: (res, ids) =>
      setImported(
        res.skipped.length
          ? tr('common.select.partial', { done: res.deleted.length, count: ids.length })
          : tr('questions.cleared', { count: res.deleted.length }),
      ),
    onError: () => setImported(tr('common.failed')),
    // Either way the outcome shows in the header's status line, which the selection hides.
    onSettled: () => {
      sel.cancel();
      qc.invalidateQueries({ queryKey: questionKeys.queue });
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
  });
  // Someone else's question can be ticked to move it, not to delete it.
  const mineSelected = sel.selected.filter((id) => data?.custom.some((q) => q.id === id && canDelete(q)));
  const askDeleteSelected = async () => {
    const count = mineSelected.length;
    const others = sel.count - count;
    if (!count) return;
    const message = others ? tr('questions.deleteSelectedOthers', { count: others }) : undefined;
    if (await confirmBatch(tr('questions.deleteSelectedTitle', { count }), tr('common.delete'), message)) removeMany.mutate(mineSelected);
  };
  const rowActions = (q: Queue['custom'][number]) => [
    ...(othersQuestion(q) ? [{ name: 'report', label: tr('moderation.reportQuestion') }] : []),
    ...(canDelete(q) ? [{ name: 'delete', label: tr('questions.deleteQuestion') }] : []),
  ];
  const row = (q: Queue['custom'][number]) => (
    <Card
      style={{
        flexDirection: 'row',
        gap: 12,
        alignItems: 'center',
        padding: 14,
      }}>
      {q.author_id != null ? <Avatar member={byId.get(q.author_id)} size={32} /> : <MessageCircleQuestion size={20} color={t.mut} />}
      <View style={{ flex: 1, gap: 2 }}>
        <QuestionText
          text={q.text}
          rank={q.kind === 'rank'}
          choose={q.kind === 'choose'}
          said={q.kind === 'said'}
          style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}
        />
        {/* A choose question's items, in one line. */}
        {!!q.items?.length && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
            {q.items.join(' · ')}
          </Text>
        )}
      </View>
      {/* Its votes will be anonymous (the room's rule leaves it to each question). */}
      {perQuestion && q.anonymous && <AnonymousMark />}
      {/* Its author or the room owner can take it out before it's asked. */}
      {canDelete(q) && (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('questions.deleteQuestion')}
          onPress={() => setToDelete(q)}
          hitSlop={8}
          style={{
            width: 34,
            height: 34,
            borderRadius: 17,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: t.card2,
          }}>
          <Trash2 size={16} color={t.mut} />
        </Pressable>
      )}
    </Card>
  );
  return (
    <Screen
      scroller={scroller}
      onScroll={tracker.onScroll}
      onContentSizeChange={tracker.onContentSizeChange}
      scrollEnabled={Platform.OS === 'web' || !dragging}
      // Only while selecting: the Screen keeps a padded strip for any footer.
      footer={
        sel.on ? (
          <SelectionBar
            sel={sel}
            actions={[
              ...(canMove
                ? [
                    { icon: ArrowUpToLine, label: tr('questions.moveFirst'), onPress: () => moveSelected(true) },
                    { icon: ArrowDownToLine, label: tr('questions.moveLast'), onPress: () => moveSelected(false) },
                  ]
                : []),
              ...(canDeleteMany
                ? [
                    {
                      icon: Trash2,
                      label: tr('common.delete'),
                      tone: 'danger' as const,
                      onPress: askDeleteSelected,
                      disabled: !mineSelected.length,
                      busy: removeMany.isPending,
                    },
                  ]
                : []),
            ]}
          />
        ) : undefined
      }>
      <Display size={30}>{tr('questions.title')}</Display>
      {locked ? (
        <Card
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 12,
            padding: 16,
          }}>
          <Lock size={18} color={t.mut} />
          <Text
            style={{
              flex: 1,
              fontFamily: t.font.regular,
              fontSize: 14,
              lineHeight: 20,
              color: t.mut,
            }}>
            {tr('questions.ownerOnly')}
          </Text>
        </Card>
      ) : null}
      {/* Packs to browse and the official deck stay open to everyone. */}
      {locked && community && (
        <View style={{ flexDirection: 'row', gap: 10 }}>
          <BulkButton label={tr('questions.packs')} icon={<Package size={16} color={t.txt} />} onPress={() => router.push('/packs')} />
          <BulkButton label={tr('community.entry')} icon={<Lightbulb size={16} color={t.txt} />} onPress={() => router.push('/suggest')} />
        </View>
      )}
      {/* While selecting, the list is all there is. */}
      {locked || sel.on ? null : (
        <>
          <Tip id="questions" text={tr('questions.tip')} />
          {/* Write one: the add button beside the text, the kind under it, in the same card. */}
          <Card style={{ paddingBottom: 10 }}>
            <View style={{ flexDirection: 'row', alignItems: 'flex-end', paddingRight: 10 }}>
              <TextInput
                ref={composer}
                accessibilityLabel={tr('questions.newQuestion')}
                placeholder={
                  kind === 'said'
                    ? tr('said.composer.placeholder')
                    : choose
                      ? tr('questions.placeholderChoose')
                      : rank
                        ? tr('questions.placeholderRank')
                        : tr('questions.placeholder')
                }
                placeholderTextColor={t.mut}
                value={text}
                onChangeText={setText}
                multiline
                maxLength={200}
                {...sendOnEnter(() => ok && add.mutate())}
                style={{
                  flex: 1,
                  minWidth: 0,
                  minHeight: 76,
                  padding: 16,
                  paddingBottom: 6,
                  fontFamily: t.font.regular,
                  fontSize: 16,
                  color: t.txt,
                  textAlignVertical: 'top',
                }}
              />
              <PressableScale
                accessibilityRole="button"
                accessibilityLabel={tr('common.add')}
                disabled={!ok}
                onPress={() => add.mutate()}
                style={{
                  width: 44,
                  height: 44,
                  borderRadius: 22,
                  backgroundColor: ok ? t.acc : t.card2,
                  alignItems: 'center',
                  justifyContent: 'center',
                  marginBottom: 6,
                }}>
                <Plus size={24} color={ok ? t.accTxt : t.mut} />
              </PressableScale>
            </View>
            {/* A choose question's items: 2 to 4, one per line. */}
            {choose && (
              <View style={{ paddingHorizontal: 12, paddingBottom: 10, gap: 8 }}>
                {items.map((item, i) => (
                  <View key={i} style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                    <TextInput
                      accessibilityLabel={tr('questions.item', { n: i + 1 })}
                      placeholder={tr('questions.item', { n: i + 1 })}
                      placeholderTextColor={t.mut}
                      value={item}
                      onChangeText={(v) => setItem(i, v)}
                      maxLength={ITEM_MAX}
                      returnKeyType="send"
                      submitBehavior="blurAndSubmit"
                      onSubmitEditing={() => ok && add.mutate()}
                      style={{
                        flex: 1,
                        height: 44,
                        paddingHorizontal: 12,
                        borderRadius: 12,
                        backgroundColor: t.card2,
                        fontFamily: t.font.regular,
                        fontSize: 16,
                        color: t.txt,
                      }}
                    />
                    {items.length > ITEMS_MIN && (
                      <Pressable
                        accessibilityRole="button"
                        accessibilityLabel={tr('questions.removeItem', { n: i + 1 })}
                        onPress={() => setItems((l) => l.filter((_, j) => j !== i))}
                        hitSlop={6}
                        style={{ width: 34, height: 34, borderRadius: 17, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
                        <X size={16} color={t.mut} />
                      </Pressable>
                    )}
                  </View>
                ))}
                <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, minHeight: 32 }}>
                  {items.length < ITEMS_MAX && (
                    <Pressable
                      accessibilityRole="button"
                      onPress={() => setItems((l) => [...l, ''])}
                      hitSlop={6}
                      style={{ flexDirection: 'row', alignItems: 'center', gap: 6, paddingVertical: 6, paddingHorizontal: 4 }}>
                      <Plus size={16} color={t.accInk} />
                      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>{tr('questions.addItem')}</Text>
                    </Pressable>
                  )}
                  {(sameItems || addError) && (
                    <Text numberOfLines={1} style={{ flex: 1, textAlign: 'right', fontFamily: t.font.regular, fontSize: 13, color: t.tagInk }}>
                      {sameItems ? tr('questions.sameItems') : addError}
                    </Text>
                  )}
                </View>
              </View>
            )}
            {/* The next question: about us, a ranking (everyone, first to last), or a choose (who'd take which item). */}
            <View style={{ paddingHorizontal: 10, flexDirection: 'row', alignItems: 'center', gap: 8 }}>
              <View style={{ flex: 1 }}>
                <Segmented
                  fill
                  label={tr('questions.kind')}
                  value={rank ? 'rank' : choose ? 'choose' : kind === 'said' ? 'said' : 'pick'}
                  onChange={(k) => {
                    setRank(k === 'rank');
                    setChoose(k === 'choose');
                    setSaid(k === 'said');
                    setAddError(null);
                  }}
                  options={[
                    { id: 'pick', name: tr('vote.kind.pick') },
                    { id: 'rank', name: tr('vote.kind.rank') },
                    ...(features.choose ? [{ id: 'choose' as const, name: tr('vote.kind.choose') }] : []),
                    ...(features.said ? [{ id: 'said' as const, name: tr('said.short') }] : []),
                  ]}
                />
              </View>
              {/* A choose question is never anonymous: who took what is its whole point. Nor a said one (no votes on people). */}
              {perQuestion && kind !== 'choose' && kind !== 'said' && <AnonymousToggle value={anonymous} onChange={setAnon} />}
            </View>
          </Card>
          {/* Or many at once. */}
          <View style={{ flexDirection: 'row', gap: 10 }}>
            <BulkButton label={tr('questions.packs')} icon={<Package size={16} color={t.txt} />} onPress={() => router.push('/packs')} />
            <BulkButton
              label={importFile.isPending ? tr('questions.importing') : tr('questions.import')}
              icon={<FileUp size={16} color={t.txt} />}
              accessibilityLabel={tr('questions.importFile')}
              disabled={importFile.isPending}
              onPress={pick}
            />
            {/* A question for the official deck, voted on by everyone. */}
            {community && (
              <BulkButton label={tr('community.entry')} icon={<Lightbulb size={16} color={t.txt} />} onPress={() => router.push('/suggest')} />
            )}
          </View>
          {/* The file format, for an AI to write a question file. */}
          {features.packTemplate && <TemplateLink style={{ alignSelf: 'center', marginTop: -6 }} />}
        </>
      )}
      {draft && !sel.on && (
        <Card style={{ padding: 16, gap: 12 }}>
          <View
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              justifyContent: 'space-between',
            }}>
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>
              {tr('questions.toImport', { chosen: chosen.length, total: draft.length })}
            </Text>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={allOn ? tr('questions.uncheckAll') : tr('questions.checkAll')}
              onPress={toggleAll}
              hitSlop={8}>
              <Text
                style={{
                  fontFamily: t.font.semibold,
                  fontSize: 14,
                  color: t.accInk,
                }}>
                {allOn ? tr('questions.none') : tr('questions.all')}
              </Text>
            </Pressable>
          </View>
          {/* Ask them in a random order instead of the file's. */}
          <Toggle icon={Shuffle} label={tr('questions.randomOrder')} value={mix} onChange={setMix} />
          {draft.map((l, i) => (
            <Pressable
              key={i}
              accessibilityRole="checkbox"
              accessibilityState={{
                checked: l.selected,
                disabled: !!l.problem,
              }}
              onPress={() => toggle(i)}
              style={{
                flexDirection: 'row',
                gap: 10,
                alignItems: 'flex-start',
                opacity: l.problem ? 0.5 : 1,
              }}>
              <View
                style={{
                  width: 22,
                  height: 22,
                  borderRadius: 7,
                  marginTop: 1,
                  alignItems: 'center',
                  justifyContent: 'center',
                  backgroundColor: l.selected ? t.acc : 'transparent',
                  borderWidth: l.selected ? 0 : 2,
                  borderColor: t.line,
                }}>
                {l.selected && <Check size={14} color={t.accTxt} strokeWidth={3} />}
              </View>
              {l.rank && <Trophy size={14} color={t.accInk} style={{ marginTop: 3 }} />}
              {l.kind === 'choose' && <Cable size={14} color={t.accInk} style={{ marginTop: 3 }} />}
              {l.anonymous && features.anonymousVotes && (
                <View style={{ marginTop: 3 }}>
                  <AnonymousMark />
                </View>
              )}
              <View style={{ flex: 1 }}>
                <Text
                  style={{
                    fontFamily: t.font.regular,
                    fontSize: 14,
                    lineHeight: 20,
                    color: t.txt,
                  }}>
                  {tidy(l.text)}
                  {l.problem && <Text style={{ color: t.tagInk }}>{`  · ${tr(`questions.problem.${l.problem}`)}`}</Text>}
                </Text>
                {!!l.items?.length && (
                  <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
                    {l.items.join(' · ')}
                  </Text>
                )}
              </View>
            </Pressable>
          ))}
          <View style={{ flexDirection: 'row', gap: 10 }}>
            <Pressable
              accessibilityRole="button"
              onPress={() => setDraft(null)}
              style={{
                flex: 1,
                height: 48,
                borderRadius: t.radius.pill,
                alignItems: 'center',
                justifyContent: 'center',
                flexDirection: 'row',
                gap: 6,
                backgroundColor: t.card2,
              }}>
              <X size={18} color={t.txt} />
              <Text
                style={{
                  fontFamily: t.font.semibold,
                  fontSize: 15,
                  color: t.txt,
                }}>
                {tr('common.cancel')}
              </Text>
            </Pressable>
            <Pressable
              accessibilityRole="button"
              disabled={!chosen.length || importFile.isPending}
              onPress={() => importFile.mutate(mix ? shuffled(chosen) : chosen)}
              style={{
                flex: 2,
                height: 48,
                borderRadius: t.radius.pill,
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: chosen.length ? t.acc : t.card2,
              }}>
              <Text
                style={{
                  fontFamily: t.font.bold,
                  fontSize: 15,
                  color: chosen.length ? t.accTxt : t.mut,
                }}>
                {importFile.isPending ? tr('questions.importing') : tr('questions.importCount', { count: chosen.length })}
              </Text>
            </Pressable>
          </View>
        </Card>
      )}
      {/* What's coming: count, last action, shuffle, select; while selecting, how many are ticked. */}
      {sel.on ? (
        <View style={{ marginTop: 6 }}>
          <SelectionHeader sel={sel} />
        </View>
      ) : (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 6 }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{tr('questions.upcoming')}</Text>
          <View style={{ paddingHorizontal: 8, height: 22, borderRadius: 11, justifyContent: 'center', backgroundColor: t.card2 }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>{data?.custom.length ?? 0}</Text>
          </View>
          <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
            {imported ?? ''}
          </Text>
          {!locked && (data?.custom.length ?? 0) > 1 && (
            <PressableScale
              accessibilityRole="button"
              accessibilityLabel={tr('questions.shuffleUpcoming')}
              disabled={shuffle.isPending}
              onPress={() => shuffle.mutate()}
              hitSlop={8}
              style={{
                width: 36,
                height: 36,
                borderRadius: 18,
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: t.accSoft,
                opacity: shuffle.isPending ? 0.5 : 1,
              }}>
              <Shuffle size={18} color={t.accInk} />
            </PressableScale>
          )}
          {clearQueue && group?.my_role === 'owner' && (data?.custom.length ?? 0) > 0 && (
            <PressableScale
              accessibilityRole="button"
              accessibilityLabel={tr('questions.clearQueue')}
              disabled={clear.isPending}
              onPress={askClear}
              hitSlop={8}
              style={{
                width: 36,
                height: 36,
                borderRadius: 18,
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: t.card2,
                opacity: clear.isPending ? 0.5 : 1,
              }}>
              <ListX size={18} color={t.mut} />
            </PressableScale>
          )}
          {canSelect && !!selectable?.length && <SelectButton size={36} onPress={() => sel.start()} />}
        </View>
      )}
      <QueryState query={queue} />
      {/* Hold a question to drag it elsewhere; held and let go on someone else's: report it.
          Selecting swaps it for the plain list (and back) at once: the old list's rows don't fade out. */}
      {sortable && data && !sel.on ? (
        <LayoutAnimationConfig skipExiting>
          <View>
            <ReorderList
              items={data.custom}
              render={row}
              isNew={isNew}
              scroller={scroller}
              tracker={tracker}
              onDragChange={setDragging}
              onReorder={reordered}
              onStill={(id) => {
                const q = data.custom.find((x) => x.id === id);
                if (q && othersQuestion(q)) setToReport({ kind: 'question', id });
              }}
            />
          </View>
        </LayoutAnimationConfig>
      ) : data ? (
        <LayoutAnimationConfig skipExiting>
          <View style={{ gap: 16 }}>
            {/* Deleting one closes the gap and shuffling slides every card to its new place. */}
            {data.custom.map((q) => (
              <Animated.View key={q.id} entering={isNew(q.id) ? motion.enter : undefined} exiting={motion.exit} layout={motion.layout}>
                {sel.on ? (
                  <SelectableRow sel={sel} id={q.id} label={tidy(q.text)} disabled={!selectable?.includes(q.id)}>
                    {row(q)}
                  </SelectableRow>
                ) : (
                  <Pressable
                    delayLongPress={400}
                    onLongPress={othersQuestion(q) ? () => setToReport({ kind: 'question', id: q.id }) : undefined}
                    // Screen readers can't long-press: the report (and the delete button inside the row) as actions.
                    accessibilityActions={rowActions(q)}
                    onAccessibilityAction={(e) => {
                      if (e.nativeEvent.actionName === 'report') setToReport({ kind: 'question', id: q.id });
                      if (e.nativeEvent.actionName === 'delete') setToDelete(q);
                    }}>
                    {row(q)}
                  </Pressable>
                )}
              </Animated.View>
            ))}
          </View>
        </LayoutAnimationConfig>
      ) : null}
      <ModerationDialog target={toReport} onClose={() => setToReport(null)} />
      <ConfirmModal
        visible={!!toDelete}
        title={tr('questions.deleteTitle')}
        message={toDelete ? tr('questions.deleteBody', { text: tidy(toDelete.text) }) : undefined}
        confirmLabel={tr('common.delete')}
        busy={remove.isPending}
        onCancel={() => setToDelete(null)}
        onConfirm={() => toDelete && remove.mutate(toDelete.id)}
      />
    </Screen>
  );
}

/** Fisher–Yates: a random order for the imported lines. */
function shuffled<T>(items: T[]): T[] {
  const out = [...items];
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

/** One of the buttons under the composer (packs, import, suggest). */
function BulkButton({
  label,
  icon,
  onPress,
  disabled,
  accessibilityLabel,
}: {
  label: string;
  icon: React.ReactNode;
  onPress: () => void;
  disabled?: boolean;
  accessibilityLabel?: string;
}) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={accessibilityLabel ?? label}
      disabled={disabled}
      onPress={onPress}
      style={{
        flex: 1,
        height: 44,
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 8,
        borderRadius: t.radius.pill,
        backgroundColor: t.card,
        borderWidth: 1.5,
        borderColor: t.line,
        opacity: disabled ? 0.5 : 1,
      }}>
      {icon}
      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>
        {label}
      </Text>
    </PressableScale>
  );
}
