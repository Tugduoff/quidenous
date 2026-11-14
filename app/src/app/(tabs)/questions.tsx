import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { Cable, Check, FileUp, Lightbulb, Lock, MessageCircleQuestion, Package, Plus, Shuffle, Trash2, Trophy, X } from 'lucide-react-native';
import { type ComponentRef, useEffect, useRef, useState } from 'react';
import { Platform, Pressable, type ScrollView, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { ModerationDialog, type ModTarget } from '@/components/moderation';
import { ReorderList, useScrollTracker } from '@/components/reorder-list';
import { Screen } from '@/components/screen';
import { Tip } from '@/components/tip';
import { Avatar, Card, ConfirmModal, Display, PressableScale, QuestionText, Toggle } from '@/components/ui';
import { api, ApiError, type Queue, type QuestionKind } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { useT } from '@/lib/i18n';
import { ITEM_MAX, ITEMS_MAX, ITEMS_MIN, itemsOk, pickQuestionFile, splitTag, takeHandedDraft, type DraftLine } from '@/lib/import-draft';
import { motion, useArrivals } from '@/lib/motion';
import { keys, useGroup, useMembers, useQueue } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { sendOnEnter } from '@/lib/submit';

const MIN = 8;

export default function QuestionsScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { data } = useQueue();
  // The queue as it first loads just shows; questions added afterwards fade in.
  const isNew = useArrivals(data?.custom.map((q) => q.id));
  const { data: group } = useGroup();
  const { byId } = useMembers();
  const [text, setText] = useState('');
  const [rank, setRank] = useState(false);
  // "Qui choisit quoi": the question and its 2 to 4 items.
  const [choose, setChoose] = useState(false);
  const [items, setItems] = useState<string[]>(['', '']);
  const [addError, setAddError] = useState<string | null>(null);
  const { me } = useSession();
  const { community, reorder: canReorder } = useFeatures();
  const [toDelete, setToDelete] = useState<{ id: number; text: string } | null>(null);
  // Long-press on someone else's question: report it.
  const [toReport, setToReport] = useState<ModTarget | null>(null);
  const remove = useMutation({
    mutationFn: (id: number) => api(`/questions/${id}`, { method: 'DELETE' }),
    onSettled: () => {
      setToDelete(null);
      qc.invalidateQueries({ queryKey: keys.queue });
    },
  });
  // Typing `[classement] …` (or `[rank] …`, `[choix] … | A | B`) by hand works too.
  const typed = splitTag(text.trim());
  const kind: QuestionKind = choose || typed.kind === 'choose' ? 'choose' : rank || typed.rank ? 'rank' : 'pick';
  const chosenItems = choose ? items.map((i) => i.trim()) : (typed.items ?? []);
  const sameItems = new Set(chosenItems.filter(Boolean).map((i) => i.toLowerCase())).size < chosenItems.filter(Boolean).length;
  const add = useMutation({
    mutationFn: () =>
      api('/questions', {
        method: 'POST',
        json: kind === 'choose' ? { text: typed.text, kind, items: chosenItems } : { text: typed.text, kind },
      }),
    onSuccess: () => {
      setText('');
      setRank(false);
      setChoose(false);
      setItems(['', '']);
      setAddError(null);
      qc.invalidateQueries({ queryKey: keys.queue });
      qc.invalidateQueries({ queryKey: keys.current });
    },
    onError: (e) => setAddError(e instanceof ApiError && e.code === 'bad_items' ? tr('questions.badItems') : null),
  });
  const ok = typed.text.length >= MIN && (kind !== 'choose' || itemsOk(chosenItems)) && !add.isPending;
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
    mutationFn: (lines: string[]) =>
      api<{ added: number; skipped: number }>('/questions/import', {
        method: 'POST',
        json: { text: lines.join('\n') },
      }),
    onSuccess: (res) => {
      setDraft(null);
      setImported(
        res.skipped ? tr('questions.importedSkipped', { added: res.added, skipped: res.skipped }) : tr('questions.imported', { added: res.added }),
      );
      qc.invalidateQueries({ queryKey: keys.queue });
      qc.invalidateQueries({ queryKey: keys.current });
    },
    onError: () => setImported(tr('questions.importFailed')),
  });
  const chosen = draft?.filter((l) => l.selected).map((l) => l.line) ?? [];
  // Shuffle the whole upcoming queue (questions added later still go last).
  const shuffle = useMutation({
    mutationFn: () => api<{ shuffled: number }>('/questions/shuffle', { method: 'POST' }),
    onSuccess: () => {
      setImported(tr('questions.shuffled'));
      qc.invalidateQueries({ queryKey: keys.queue });
    },
    onError: () => setImported(tr('questions.shuffleFailed')),
  });
  // Hold a question and drag it: the new order shows at once, and comes back if the server says no.
  const scroller = useRef<ComponentRef<typeof ScrollView>>(null);
  const tracker = useScrollTracker();
  const [dragging, setDragging] = useState(false);
  const reorder = useMutation({
    mutationFn: (ids: number[]) => api('/questions/order', { method: 'POST', json: { ids } }),
    onError: () => setImported(tr('questions.reorderFailed')),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.queue }),
  });
  const reordered = (ids: number[]) => {
    const before = qc.getQueryData<Queue>(keys.queue);
    qc.cancelQueries({ queryKey: keys.queue });
    if (before) {
      const at = new Map(ids.map((id, i) => [id, i]));
      const custom = [...before.custom].sort((a, b) => (at.get(a.id) ?? Infinity) - (at.get(b.id) ?? Infinity));
      qc.setQueryData<Queue>(keys.queue, { ...before, custom });
    }
    reorder.mutate(ids, { onError: () => before && qc.setQueryData(keys.queue, before) });
  };
  const toggle = (i: number) => setDraft((d) => d && d.map((l, j) => (j === i && !l.problem ? { ...l, selected: !l.selected } : l)));
  const allOn = !!draft && draft.every((l) => l.selected || l.problem);
  const toggleAll = () => setDraft((d) => d && d.map((l) => (l.problem ? l : { ...l, selected: !allOn })));

  // The owner can keep questions to themselves (room setting).
  const locked = group?.settings.questions_by === 'owner' && group.my_role !== 'owner';
  const sortable = canReorder && !locked && (data?.custom.length ?? 0) > 1;
  const othersQuestion = (q: Queue['custom'][number]) => q.author_id != null && q.author_id !== me?.id;
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
          style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}
        />
        {/* A choose question's items, in one line. */}
        {!!q.items?.length && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
            {q.items.join(' · ')}
          </Text>
        )}
      </View>
      {/* Its author or the room owner can take it out before it's asked. */}
      {(q.author_id === me?.id || group?.my_role === 'owner') && (
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
      scrollEnabled={Platform.OS === 'web' || !dragging}>
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
      {locked ? null : (
        <>
          <Tip id="questions" text={tr('questions.tip')} />
          {/* Write one: the ranking switch and the add button sit under the text, in the same card. */}
          <Card style={{ paddingBottom: 10 }}>
            <TextInput
              ref={composer}
              accessibilityLabel={tr('questions.newQuestion')}
              placeholder={choose ? tr('questions.placeholderChoose') : rank ? tr('questions.placeholderRank') : tr('questions.placeholder')}
              placeholderTextColor={t.mut}
              value={text}
              onChangeText={setText}
              multiline
              maxLength={200}
              {...sendOnEnter(() => ok && add.mutate())}
              style={{
                minHeight: 76,
                padding: 16,
                paddingBottom: 6,
                fontFamily: t.font.regular,
                fontSize: 16,
                color: t.txt,
                textAlignVertical: 'top',
              }}
            />
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
                    <Text numberOfLines={1} style={{ flex: 1, textAlign: 'right', fontFamily: t.font.regular, fontSize: 13, color: t.tag }}>
                      {sameItems ? tr('questions.sameItems') : addError}
                    </Text>
                  )}
                </View>
              </View>
            )}
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 14, paddingLeft: 16, paddingRight: 10 }}>
              {/* The next question is a ranking (everyone, first to last), or a choose (who'd take which item). */}
              <Toggle
                icon={Trophy}
                label={tr('questions.ranking')}
                value={rank}
                onChange={(v) => {
                  setRank(v);
                  if (v) setChoose(false);
                }}
              />
              <Toggle
                icon={Cable}
                label={tr('questions.choosing')}
                a11y={tr('questions.choosingLabel')}
                value={choose}
                onChange={(v) => {
                  setChoose(v);
                  setAddError(null);
                  if (v) setRank(false);
                }}
              />
              <View style={{ flex: 1 }} />
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
                }}>
                <Plus size={24} color={ok ? t.accTxt : t.mut} />
              </PressableScale>
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
            {community && <BulkButton label={tr('community.entry')} icon={<Lightbulb size={16} color={t.txt} />} onPress={() => router.push('/suggest')} />}
          </View>
        </>
      )}
      {draft && (
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
              <View style={{ flex: 1 }}>
                <Text
                  style={{
                    fontFamily: t.font.regular,
                    fontSize: 14,
                    lineHeight: 20,
                    color: t.txt,
                  }}>
                  {l.text}
                  {l.problem && <Text style={{ color: t.tag }}>{`  · ${tr(`questions.problem.${l.problem}`)}`}</Text>}
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
      {/* What's coming: count, last action, shuffle. */}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 6 }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{tr('questions.upcoming')}</Text>
        <View style={{ paddingHorizontal: 8, height: 22, borderRadius: 11, justifyContent: 'center', backgroundColor: t.card2 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>{(data?.custom.length ?? 0) + (data?.deck_left ?? 0)}</Text>
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
      </View>
      {/* Hold a question to drag it elsewhere; held and let go on someone else's: report it. */}
      {sortable && data ? (
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
      ) : (
        // Deleting one closes the gap and shuffling slides every card to its new place.
        data?.custom.map((q) => (
          <Animated.View key={q.id} entering={isNew(q.id) ? motion.enter : undefined} exiting={motion.exit} layout={motion.layout}>
            <Pressable delayLongPress={400} onLongPress={othersQuestion(q) ? () => setToReport({ kind: 'question', id: q.id }) : undefined}>
              {row(q)}
            </Pressable>
          </Animated.View>
        ))
      )}
      <ModerationDialog target={toReport} onClose={() => setToReport(null)} />
      <ConfirmModal
        visible={!!toDelete}
        title={tr('questions.deleteTitle')}
        message={toDelete ? tr('questions.deleteBody', { text: toDelete.text.replace(/ ([?!:;»])/g, '\u00A0$1') }) : undefined}
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
