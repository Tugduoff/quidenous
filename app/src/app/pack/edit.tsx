import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { FileUp, Flame, Plus, Trophy, X } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { Chip, PageHeader } from '@/components/pack-card';
import { Screen } from '@/components/screen';
import { Button, Card, Display, PressableScale, Segmented, Toggle } from '@/components/ui';
import { api, ApiError } from '@/lib/api';
import { pickQuestionFile } from '@/lib/import-draft';
import { getLang, type Lang, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { isPackTag, MAX_TAGS, PACK_TAGS, packError, packKeys, type PackQuestion, type PackTag, TAG_ICONS, usePackDetail } from '@/lib/packs';
import { useGroup, useQueue } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

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

  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');
  const [tags, setTags] = useState<PackTag[]>([]);
  const [spicyPick, setSpicy] = useState(false);
  const [lang, setLang] = useState<Lang | null>(null);
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
  } else if (!seeded && id == null && queue) {
    setSeeded(true);
    setQuestions((qs) => merge(qs, queue.custom));
  }

  const spicy = spicyPick || tags.includes('pimente');
  const language: Lang = lang ?? group?.language ?? getLang();
  const keyOf = (text: string) => text.trim().toLowerCase();
  const draftOk = [...draft.trim()].length >= MIN;

  const add = () => {
    if (!draftOk) return;
    setQuestions((qs) => merge(qs, [{ text: draft.trim(), kind: 'pick' }]));
    setDraft('');
  };
  const importFile = async () => {
    const lines = await pickQuestionFile().catch(() => null);
    if (!lines) return;
    if (lines === 'empty') return setNote(tr('packs.emptyFile'));
    setNote(null);
    setQuestions((qs) =>
      merge(
        qs,
        lines.filter((l) => !l.problem).map((l) => ({ text: l.text, kind: l.rank ? 'rank' : 'pick' })),
      ),
    );
  };
  const toggleTag = (tag: PackTag) => setTags((l) => (l.includes(tag) ? l.filter((x) => x !== tag) : l.length < MAX_TAGS ? [...l, tag] : l));

  const save = useMutation({
    mutationFn: () => {
      const body = { name: name.trim(), description: desc.trim(), language, tags, spicy, questions };
      return id != null
        ? api<{ id: number }>(`/packs/${id}`, { method: 'PUT', json: body })
        : api<{ id: number; count: number }>('/packs', { method: 'POST', json: body });
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
        <>
          {note && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag, textAlign: 'center' }}>{note}</Text>}
          <Button disabled={!name.trim() || !questions.length || (id != null && !seeded)} loading={save.isPending} onPress={() => save.mutate()}>
            {tr(id != null ? 'packs.save' : 'packs.publish')}
          </Button>
        </>
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

      <View style={{ flexDirection: 'row', alignItems: 'center', marginTop: 4 }}>
        <Text style={[label, { flex: 1 }]}>{tr('packs.questions', { count: questions.length })}</Text>
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

      {questions.length > 0 && (
        <Card style={{ paddingVertical: 4 }}>
          {questions.map((q, i) => {
            const rank = q.kind === 'rank';
            return (
              <Animated.View
                key={keyOf(q.text)}
                entering={motion.enter}
                exiting={motion.exit}
                layout={motion.layout}
                style={{
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 6,
                  paddingLeft: 6,
                  paddingRight: 6,
                  borderTopWidth: i ? 1 : 0,
                  borderColor: t.line,
                }}>
                <Pressable
                  accessibilityRole="switch"
                  accessibilityLabel={tr('packs.ranking')}
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
                <Text style={{ flex: 1, paddingVertical: 10, fontFamily: t.font.regular, fontSize: 14, lineHeight: 20, color: t.txt }}>{q.text}</Text>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel={tr('packs.removeQuestion')}
                  hitSlop={4}
                  onPress={() => setQuestions((qs) => qs.filter((_, j) => j !== i))}
                  style={{ width: 36, height: 36, alignItems: 'center', justifyContent: 'center' }}>
                  <X size={16} color={t.mut} />
                </Pressable>
              </Animated.View>
            );
          })}
        </Card>
      )}
    </Screen>
  );
}

/** `more` after `list`, without a question already there (case-insensitive) or out of bounds. */
function merge(list: PackQuestion[], more: PackQuestion[]): PackQuestion[] {
  const seen = new Set(list.map((q) => q.text.trim().toLowerCase()));
  const out = [...list];
  for (const q of more) {
    const text = q.text.trim();
    const key = text.toLowerCase();
    const len = [...text].length;
    if (seen.has(key) || len < MIN || len > MAX) continue;
    seen.add(key);
    out.push({ text, kind: q.kind === 'rank' ? 'rank' : 'pick' });
  }
  return out;
}
