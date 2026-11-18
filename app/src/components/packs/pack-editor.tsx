import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Trash2, Trophy } from 'lucide-react-native';
import { useState } from 'react';
import { Text } from 'react-native';
import { PageHeader } from '@/components/packs/pack-card';
import { Screen } from '@/components/ui/screen';
import { type BatchAction, confirmBatch, SelectionBar, useSelection } from '@/components/ui/selection';
import { ApiError } from '@/lib/api/client';
import { itemsOk, parseQuestions, QUESTION_MIN, splitTag } from '@/lib/utils/import-draft';
import { getLang, type Lang, useT } from '@/lib/i18n';
import { parsePackHeader } from '@/lib/utils/pack-file';
import { clip, mergeQuestions as merge, PACK_MIN, questionKey as keyOf } from '@/lib/utils/pack-draft';
import { isPackTag, MAX_TAGS, packError, packKeys, type PackQuestion, type PackTag, useCommunityMe, usePackDetail, updatePack, createPack } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Display } from '@/components/ui/text';
import { askConfirm } from '@/lib/confirm';
import { useGroup } from '@/lib/api/group';
import { useQueue } from '@/lib/api/questions';
import { useFeatures } from '@/lib/api/features';
import { pickTextFile } from '@/lib/device/files';
import { PackMetaCard } from '@/components/packs/pack-meta-card';
import { PackTagPicker } from '@/components/packs/pack-tag-picker';
import { PackQuestionTools } from '@/components/packs/pack-question-tools';
import { PackQuestionList } from '@/components/packs/pack-question-list';

export function PackEditor({ id }: { id: number | null }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { data: pack } = usePackDetail(id ?? 0);
  const { data: queue } = useQueue();
  const { data: group } = useGroup();
  const { data: me } = useCommunityMe();
  // An older server: packs without choose questions, no AI template.
  const { loaded, packChoose, packTemplate, anonymousVotes } = useFeatures();

  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');
  const [tags, setTags] = useState<PackTag[]>([]);
  const [spicyPick, setSpicy] = useState(false);
  const [lang, setLang] = useState<Lang | null>(null);
  // Admins can publish straight into the Coflop pick.
  const [featured, setFeatured] = useState(false);
  const [questions, setQuestions] = useState<PackQuestion[]>([]);
  const [draft, setDraft] = useState('');
  const [note, setNote] = useState<string | null>(null);
  // Filled once: from the pack being edited, or from the room's own questions for a new one.
  const [seeded, setSeeded] = useState(false);
  if (!seeded && id != null && pack) {
    setSeeded(true);
    setName(pack.name);
    setDesc(pack.description);
    setTags(pack.tags.filter(isPackTag));
    setSpicy(pack.spicy);
    setLang(pack.language ?? null);
    setQuestions(pack.questions);
  } else if (!seeded && id == null && queue && loaded) {
    setSeeded(true);
    // A new pack starts from the room's own questions (choose ones only when the server takes them in packs; never said ones).
    setQuestions((qs) =>
      merge(
        qs,
        queue.custom
          .flatMap((q) => (q.kind === 'said' || (!packChoose && q.kind === 'choose') ? [] : [{ ...q, kind: q.kind }]))
          .map((q) => ({ text: q.text, kind: q.kind, items: q.items, anonymous: q.anonymous })),
      ),
    );
  }

  const spicy = spicyPick || tags.includes('pimente');
  const language: Lang = lang ?? group?.language ?? getLang();
  // Several questions at once (hold one, or the button by the count): remove them, or switch them to a ranking.
  const sel = useSelection(questions.map((q) => keyOf(q.text)));
  const picked = questions.filter((q) => sel.has(keyOf(q.text)) && q.kind !== 'choose');
  // `[classement] …` and (on a server that takes them) `[choix] Question | A | B` typed by hand.
  const typed = splitTag(draft);
  const draftOk = [...typed.text].length >= QUESTION_MIN && (typed.kind !== 'choose' || (packChoose && itemsOk(typed.items ?? [])));

  const add = () => {
    if (!draftOk) return;
    setQuestions((qs) => merge(qs, [{ text: typed.text, kind: typed.kind, items: typed.items, anonymous: typed.anonymous }]));
    setDraft('');
  };
  // A file fills the list, and the pack's name, description, categories, language and 16+ from its header.
  const importFile = async () => {
    const content = await pickTextFile().catch(() => null);
    if (content == null) return;
    const head = parsePackHeader(content);
    if (head.title) setName(clip(head.title, 40));
    if (head.description) setDesc(clip(head.description, 140));
    if (head.categories.length) setTags(head.categories);
    if (head.lang) setLang(head.lang);
    if (head.spicy != null) setSpicy(head.spicy);
    const lines = parseQuestions(content);
    if (!lines.length) return setNote(tr('packs.emptyFile'));
    setNote(null);
    setQuestions((qs) =>
      merge(
        qs,
        lines
          .filter((l) => !l.problem && (packChoose || l.kind !== 'choose'))
          .map((l) => ({ text: l.text, kind: l.kind, items: l.items, anonymous: l.anonymous })),
      ),
    );
  };
  const clearAll = async () => {
    if (await askConfirm(tr('packs.clearAsk', { count: questions.length }), undefined, tr('packs.clearAll'))) setQuestions([]);
  };
  const removePicked = async () => {
    const gone = new Set(sel.selected);
    if (!(await confirmBatch(tr('packs.removeQuestions', { count: gone.size }), tr('packs.removeQuestion')))) return;
    setQuestions((qs) => qs.filter((q) => !gone.has(keyOf(q.text))));
    sel.cancel();
  };
  // All the ticked ones already a ranking: back to a pick; otherwise all become rankings. Choose questions stay as they are.
  const rankPicked = () => {
    const to = picked.every((q) => q.kind === 'rank') ? 'pick' : 'rank';
    const on = new Set(picked.map((q) => keyOf(q.text)));
    setQuestions((qs) => qs.map((q) => (on.has(keyOf(q.text)) ? { ...q, kind: to } : q)));
  };
  const actions: BatchAction[] = [
    { icon: Trophy, label: tr('vote.kind.rank'), disabled: !picked.length, onPress: rankPicked },
    { icon: Trash2, label: tr('packs.removeQuestion'), tone: 'danger', onPress: removePicked },
  ];
  const toggleTag = (tag: PackTag) => setTags((l) => (l.includes(tag) ? l.filter((x) => x !== tag) : l.length < MAX_TAGS ? [...l, tag] : l));

  const save = useMutation({
    meta: { quiet: true },
    mutationFn: () => {
      const sent = questions
        .filter((q) => packChoose || q.kind !== 'choose')
        // An older server keeps each question's anonymous flag as it was (not sent).
        .map((q) =>
          q.kind === 'choose'
            ? { text: q.text, kind: q.kind, items: q.items }
            : { text: q.text, kind: q.kind, ...(anonymousVotes ? { anonymous: !!q.anonymous } : {}) },
        );
      const body = { name: name.trim(), description: desc.trim(), language, tags, spicy, questions: sent };
      return id != null
        ? updatePack(id, body)
        : createPack(me?.admin && featured ? { ...body, featured: true } : body);
    },
    onSuccess: (r) => {
      qc.invalidateQueries({ queryKey: packKeys.all });
      if (id != null) router.back();
      else router.replace(`/pack/${r.id}`);
    },
    onError: (e) => setNote(tr(e instanceof ApiError ? packError(e.code) : 'common.failed')),
  });

  return (
    <Screen
      tabs={false}
      footer={
        sel.on ? (
          <SelectionBar sel={sel} actions={actions} />
        ) : (
          <>
            {note && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tagInk, textAlign: 'center' }}>{note}</Text>}
            {!note && questions.length < PACK_MIN && (
              <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut, textAlign: 'center' }}>
                {tr('packs.minHint', { count: questions.length, min: PACK_MIN })}
              </Text>
            )}
            <Button disabled={!name.trim() || questions.length < PACK_MIN || (id != null && !seeded)} loading={save.isPending} onPress={() => save.mutate()}>
              {tr(id != null ? 'packs.save' : 'packs.publish')}
            </Button>
          </>
        )
      }>
      <PageHeader fallback="/packs" />
      <Display size={30}>{tr(id != null ? 'packs.editTitle' : 'packs.newTitle')}</Display>

      <PackMetaCard
        name={name}
        onName={setName}
        desc={desc}
        onDesc={setDesc}
        spicy={spicy}
        spicyLocked={tags.includes('pimente')}
        onSpicy={setSpicy}
        language={language}
        onLang={setLang}
        showFeatured={id == null && !!me?.admin}
        featured={featured}
        onFeatured={setFeatured}
      />

      <PackTagPicker tags={tags} onToggle={toggleTag} />

      <PackQuestionTools
        sel={sel}
        count={questions.length}
        onClearAll={clearAll}
        onImportFile={importFile}
        packTemplate={packTemplate}
        draft={draft}
        onDraft={setDraft}
        onAdd={add}
        draftOk={draftOk}
      />

      <PackQuestionList questions={questions} setQuestions={setQuestions} sel={sel} anonymousVotes={anonymousVotes} />
    </Screen>
  );
}
