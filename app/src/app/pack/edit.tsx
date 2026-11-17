import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { Cable, FileUp, Flame, Plus, Star, Trash2, Trophy, X } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { AnonymousMark } from '@/components/anonymous';
import { Chip, PageHeader } from '@/components/pack-card';
import { Screen } from '@/components/screen';
import { type BatchAction, confirmBatch, SelectableRow, SelectButton, SelectionBar, SelectionHeader, useSelection } from '@/components/selection';
import { TemplateLink } from '@/components/template-link';
import { askConfirm, Button, Card, Display, PressableScale, Segmented, Toggle } from '@/components/ui';
import { api, ApiError } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { itemsOk, parseQuestions, pickTextFile, splitTag } from '@/lib/import-draft';
import { getLang, type Lang, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { parsePackHeader } from '@/lib/pack-file';
import {
  isPackTag,
  MAX_TAGS,
  PACK_TAGS,
  packError,
  packKeys,
  type PackQuestion,
  type PackTag,
  TAG_ICONS,
  useCommunityMe,
  usePackDetail,
} from '@/lib/packs';
import { useGroup, useQueue } from '@/lib/queries';
import { tidy } from '@/lib/text';
import { useTheme } from '@/lib/theme';

/** The server refuses a pack under 5 questions: said before the tap. */
const PACK_MIN = 5;

/** Same limits as the server for one question. */
const MIN = 8;
const MAX = 200;

/** Publish a pack (`/pack/edit`, prefilled with this room's own questions) or edit one of mine (`/pack/edit?id=`). */
export default function PackEditScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const param = useLocalSearchParams<{ id?: string }>().id;
  const id = param ? Number(param) : null;
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
  const descField = useRef<ComponentRef<typeof TextInput>>(null);

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
    // A new pack starts from the room's own questions (choose ones only when the server takes them in packs).
    setQuestions((qs) =>
      merge(
        qs,
        queue.custom
          .filter((q) => packChoose || q.kind !== 'choose')
          .map((q) => ({ text: q.text, kind: q.kind, items: q.items, anonymous: q.anonymous })),
      ),
    );
  }

  const spicy = spicyPick || tags.includes('pimente');
  const language: Lang = lang ?? group?.language ?? getLang();
  const keyOf = (text: string) => text.trim().toLowerCase();
  // Several questions at once (hold one, or the button by the count): remove them, or switch them to a ranking.
  const sel = useSelection(questions.map((q) => keyOf(q.text)));
  const picked = questions.filter((q) => sel.has(keyOf(q.text)) && q.kind !== 'choose');
  // `[classement] …` and (on a server that takes them) `[choix] Question | A | B` typed by hand.
  const typed = splitTag(draft);
  const draftOk = [...typed.text].length >= MIN && (typed.kind !== 'choose' || (packChoose && itemsOk(typed.items ?? [])));

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
        ? api<{ id: number }>(`/packs/${id}`, { method: 'PUT', json: body })
        : api<{ id: number; count: number }>('/packs', { method: 'POST', json: me?.admin && featured ? { ...body, featured: true } : body });
    },
    onSuccess: (r) => {
      qc.invalidateQueries({ queryKey: packKeys.all });
      if (id != null) router.back();
      else router.replace(`/pack/${r.id}`);
    },
    onError: (e) => setNote(tr(e instanceof ApiError ? packError(e.code) : 'common.failed')),
  });

  const input = {
    height: 44,
    paddingHorizontal: 14,
    borderRadius: 14,
    backgroundColor: t.card2,
    fontFamily: t.font.regular,
    fontSize: 16,
    color: t.txt,
  };
  const label = { fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 };

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

      <Card style={{ padding: 16, gap: 10 }}>
        <TextInput
          accessibilityLabel={tr('packs.name')}
          placeholder={tr('packs.name')}
          placeholderTextColor={t.mut}
          value={name}
          onChangeText={setName}
          maxLength={40}
          returnKeyType="next"
          submitBehavior="submit"
          onSubmitEditing={() => descField.current?.focus()}
          style={input}
        />
        <TextInput
          ref={descField}
          accessibilityLabel={tr('packs.description')}
          placeholder={tr('packs.description')}
          placeholderTextColor={t.mut}
          value={desc}
          onChangeText={setDesc}
          maxLength={140}
          multiline
          style={[input, { height: undefined, minHeight: 44, paddingTop: 11, paddingBottom: 11, textAlignVertical: 'top' }]}
        />
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12, marginTop: 4 }}>
          <Toggle label={tr('packs.spicy')} icon={Flame} value={spicy} onChange={(v) => !tags.includes('pimente') && setSpicy(v)} />
          <View style={{ flex: 1 }} />
          <Segmented<Lang>
            label={tr('packs.language')}
            options={[
              { id: 'fr', name: 'Français' },
              { id: 'en', name: 'English' },
            ]}
            value={language}
            onChange={setLang}
          />
        </View>
        {id == null && me?.admin && <Toggle label={tr('packs.featured')} icon={Star} value={featured} onChange={setFeatured} />}
      </Card>

      <Text style={label}>{tr('packs.categories')}</Text>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8, marginTop: -6 }}>
        {PACK_TAGS.map((tag) => {
          const on = tags.includes(tag);
          return (
            <Chip
              key={tag}
              icon={TAG_ICONS[tag]}
              label={tr(`packs.tags.${tag}`)}
              on={on}
              disabled={!on && tags.length >= MAX_TAGS}
              onPress={() => toggleTag(tag)}
            />
          );
        })}
      </View>

      {sel.on ? (
        <SelectionHeader sel={sel} />
      ) : (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 4 }}>
          <Text style={[label, { flex: 1 }]}>{tr('packs.questions', { count: questions.length })}</Text>
          {questions.length > 0 && <SelectButton size={32} onPress={() => sel.start()} />}
          {questions.length > 0 && (
            <PressableScale
              accessibilityRole="button"
              accessibilityLabel={tr('packs.clearAll')}
              onPress={clearAll}
              hitSlop={4}
              style={{ width: 32, height: 32, borderRadius: 16, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
              <Trash2 size={15} color={t.txt} />
            </PressableScale>
          )}
          <PressableScale
            accessibilityRole="button"
            onPress={importFile}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 6,
              height: 32,
              paddingHorizontal: 12,
              borderRadius: t.radius.pill,
              backgroundColor: t.card,
            }}>
            <FileUp size={15} color={t.txt} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.txt }}>{tr('packs.fromFile')}</Text>
          </PressableScale>
        </View>
      )}
      {/* What a file's header can fill in, and the format for an AI to write one. */}
      {!sel.on && (
        <View style={{ flexDirection: 'row', alignItems: 'flex-start', gap: 12, marginTop: -8, paddingHorizontal: 4 }}>
          <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 12, lineHeight: 17, color: t.mut }}>{tr('packs.fileHeader')}</Text>
          {packTemplate && <TemplateLink style={{ marginTop: 1 }} />}
        </View>
      )}
      {!sel.on && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 8, paddingLeft: 14, marginTop: -6 }}>
          <TextInput
            accessibilityLabel={tr('packs.addQuestion')}
            placeholder={tr('packs.addQuestion')}
            placeholderTextColor={t.mut}
            value={draft}
            onChangeText={setDraft}
            maxLength={MAX}
            returnKeyType="done"
            submitBehavior="submit"
            onSubmitEditing={add}
            style={{ flex: 1, minWidth: 0, height: 40, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
          />
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('common.add')}
            disabled={!draftOk}
            onPress={add}
            hitSlop={4}
            style={{
              width: 40,
              height: 40,
              borderRadius: 20,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: draftOk ? t.acc : t.card2,
            }}>
            <Plus size={20} color={draftOk ? t.accTxt : t.mut} />
          </PressableScale>
        </Card>
      )}

      {questions.length > 0 && (
        <Card style={{ paddingVertical: 4 }}>
          {questions.map((q, i) => {
            const rank = q.kind === 'rank';
            const choose = q.kind === 'choose';
            return (
              <Animated.View
                key={keyOf(q.text)}
                entering={motion.enter}
                exiting={motion.exit}
                layout={motion.layout}
                // Selecting: the checkbox in line with the add field's text, not against the card's edge.
                style={{ paddingLeft: sel.on ? 14 : 0, borderTopWidth: i ? 1 : 0, borderColor: t.line }}>
                <SelectableRow sel={sel} id={keyOf(q.text)} label={tidy(q.text)}>
                  {/* The hold selects; the toggle and the cross inside keep their own taps. */}
                  <Pressable
                    accessible={false}
                    onLongPress={() => sel.start(keyOf(q.text))}
                    delayLongPress={400}
                    style={{ flexDirection: 'row', alignItems: 'center', gap: 6, paddingLeft: 6, paddingRight: 6 }}>
                    {/* A choose question stays one; the others switch between pick and ranking. */}
                    {choose ? (
                      <View
                        accessibilityLabel={tr('vote.kind.choose')}
                        style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.accSoft }}>
                        <Cable size={16} color={t.accInk} />
                      </View>
                    ) : (
                      <Pressable
                        accessibilityRole="switch"
                        accessibilityLabel={tr('vote.kind.rank')}
                        accessibilityState={{ checked: rank }}
                        hitSlop={4}
                        onPress={() => setQuestions((qs) => qs.map((x, j) => (j === i ? { ...x, kind: rank ? 'pick' : 'rank' } : x)))}
                        style={{
                          width: 36,
                          height: 36,
                          borderRadius: 18,
                          alignItems: 'center',
                          justifyContent: 'center',
                          backgroundColor: rank ? t.accSoft : 'transparent',
                        }}>
                        <Trophy size={16} color={rank ? t.accInk : t.mut} style={{ opacity: rank ? 1 : 0.5 }} />
                      </Pressable>
                    )}
                    <View style={{ flex: 1, paddingVertical: 10, gap: 2 }}>
                      <Text style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 20, color: t.txt }}>{tidy(q.text)}</Text>
                      {choose && !!q.items?.length && (
                        <Text style={{ fontFamily: t.font.regular, fontSize: 12, lineHeight: 17, color: t.mut }}>{q.items.join(' · ')}</Text>
                      )}
                    </View>
                    {anonymousVotes && q.anonymous && !choose && <AnonymousMark />}
                    <Pressable
                      accessibilityRole="button"
                      accessibilityLabel={tr('packs.removeQuestion')}
                      hitSlop={4}
                      onPress={() => setQuestions((qs) => qs.filter((_, j) => j !== i))}
                      style={{ width: 36, height: 36, alignItems: 'center', justifyContent: 'center' }}>
                      <X size={16} color={t.mut} />
                    </Pressable>
                  </Pressable>
                </SelectableRow>
              </Animated.View>
            );
          })}
        </Card>
      )}
    </Screen>
  );
}

/** `more` after `list`, without a question already there (case-insensitive), out of bounds, or a choose one with bad items. */
function merge(list: PackQuestion[], more: PackQuestion[]): PackQuestion[] {
  const seen = new Set(list.map((q) => q.text.trim().toLowerCase()));
  const out = [...list];
  for (const q of more) {
    const text = q.text.trim();
    const key = text.toLowerCase();
    const len = [...text].length;
    if (seen.has(key) || len < MIN || len > MAX) continue;
    if (q.kind === 'choose') {
      const items = (q.items ?? []).map((x) => x.trim());
      if (!itemsOk(items)) continue;
      out.push({ text, kind: 'choose', items });
    } else out.push({ text, kind: q.kind === 'rank' ? 'rank' : 'pick', anonymous: !!q.anonymous });
    seen.add(key);
  }
  return out;
}

/** The first `max` characters (whole characters, not UTF-16 halves). */
const clip = (s: string, max: number) => [...s.trim()].slice(0, max).join('');
