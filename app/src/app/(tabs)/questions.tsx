import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { Check, FileUp, Lock, MessageCircleQuestion, Package, Plus, Shuffle, Trash2, Trophy, X } from 'lucide-react-native';
import { type ComponentRef, useEffect, useRef, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { ModerationDialog, type ModTarget } from '@/components/moderation';
import { Screen } from '@/components/screen';
import { Tip } from '@/components/tip';
import { Avatar, Card, ConfirmModal, Display, PressableScale, QuestionText, Toggle } from '@/components/ui';
import { api } from '@/lib/api';
import { pickQuestionFile, splitTag, takeHandedDraft, type DraftLine } from '@/lib/import-draft';
import { motion, useArrivals } from '@/lib/motion';
import { keys, useGroup, useMembers, useQueue } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { sendOnEnter } from '@/lib/submit';

const MIN = 8;

export default function QuestionsScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data } = useQueue();
  // The queue as it first loads just shows; questions added afterwards fade in.
  const isNew = useArrivals(data?.custom.map((q) => q.id));
  const { data: group } = useGroup();
  const { byId } = useMembers();
  const [text, setText] = useState('');
  const [rank, setRank] = useState(false);
  const { me } = useSession();
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
  // Typing `[classement] …` by hand works too.
  const typed = splitTag(text.trim());
  const add = useMutation({
    mutationFn: () =>
      api('/questions', {
        method: 'POST',
        json: { text: typed.text, kind: rank || typed.rank ? 'rank' : 'pick' },
      }),
    onSuccess: () => {
      setText('');
      setRank(false);
      qc.invalidateQueries({ queryKey: keys.queue });
      qc.invalidateQueries({ queryKey: keys.current });
    },
  });
  const ok = text.trim().length >= MIN && !add.isPending;
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
    if (lines === 'empty') setImported('Fichier vide');
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
      setImported(`+${res.added}` + (res.skipped ? ` · ${res.skipped} déjà là` : ''));
      qc.invalidateQueries({ queryKey: keys.queue });
      qc.invalidateQueries({ queryKey: keys.current });
    },
    onError: () => setImported('Import raté'),
  });
  const chosen = draft?.filter((l) => l.selected).map((l) => l.line) ?? [];
  // Shuffle the whole upcoming queue (questions added later still go last).
  const shuffle = useMutation({
    mutationFn: () => api<{ shuffled: number }>('/questions/shuffle', { method: 'POST' }),
    onSuccess: () => {
      setImported('Mélangé');
      qc.invalidateQueries({ queryKey: keys.queue });
    },
    onError: () => setImported('Mélange raté'),
  });
  const toggle = (i: number) => setDraft((d) => d && d.map((l, j) => (j === i && !l.problem ? { ...l, selected: !l.selected } : l)));
  const allOn = !!draft && draft.every((l) => l.selected || l.problem);
  const toggleAll = () => setDraft((d) => d && d.map((l) => (l.problem ? l : { ...l, selected: !allOn })));

  // The owner can keep questions to themselves (room setting).
  const locked = group?.settings.questions_by === 'owner' && group.my_role !== 'owner';
  return (
    <Screen>
      <Display size={30}>Questions</Display>
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
            Seul le propriétaire en ajoute.
          </Text>
        </Card>
      ) : (
        <>
          <Tip id="questions" text="Ajoute une question ou un pack." />
          {/* Write one: the ranking switch and the add button sit under the text, in the same card. */}
          <Card style={{ paddingBottom: 10 }}>
            <TextInput
              ref={composer}
              accessibilityLabel="Nouvelle question"
              placeholder={rank ? 'Classe-nous du plus… au moins…' : 'Qui de nous…'}
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
            <View style={{ flexDirection: 'row', alignItems: 'center', paddingLeft: 16, paddingRight: 10 }}>
              {/* The next question is a ranking (everyone, first to last). */}
              <Toggle icon={Trophy} label="Classement" value={rank} onChange={setRank} />
              <View style={{ flex: 1 }} />
              <PressableScale
                accessibilityRole="button"
                accessibilityLabel="Ajouter"
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
            <BulkButton label="Packs" icon={<Package size={16} color={t.txt} />} onPress={() => router.push('/packs')} />
            <BulkButton
              label={importFile.isPending ? 'Import…' : 'Importer'}
              icon={<FileUp size={16} color={t.txt} />}
              accessibilityLabel="Importer un fichier de questions"
              disabled={importFile.isPending}
              onPress={pick}
            />
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
              {chosen.length} / {draft.length} à importer
            </Text>
            <Pressable accessibilityRole="button" accessibilityLabel={allOn ? 'Tout décocher' : 'Tout cocher'} onPress={toggleAll} hitSlop={8}>
              <Text
                style={{
                  fontFamily: t.font.semibold,
                  fontSize: 14,
                  color: t.accInk,
                }}>
                {allOn ? 'Aucune' : 'Toutes'}
              </Text>
            </Pressable>
          </View>
          {/* Ask them in a random order instead of the file's. */}
          <Toggle icon={Shuffle} label="Ordre aléatoire" value={mix} onChange={setMix} />
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
              <Text
                style={{
                  flex: 1,
                  fontFamily: t.font.regular,
                  fontSize: 14,
                  lineHeight: 20,
                  color: t.txt,
                }}>
                {l.text}
                {l.problem && <Text style={{ color: t.tag }}>{`  · ${l.problem}`}</Text>}
              </Text>
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
                Annuler
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
                {importFile.isPending ? 'Import…' : `Importer ${chosen.length}`}
              </Text>
            </Pressable>
          </View>
        </Card>
      )}
      {/* What's coming: count, last action, shuffle. */}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 6 }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>À venir</Text>
        <View style={{ paddingHorizontal: 8, height: 22, borderRadius: 11, justifyContent: 'center', backgroundColor: t.card2 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>{(data?.custom.length ?? 0) + (data?.deck_left ?? 0)}</Text>
        </View>
        <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {imported ?? ''}
        </Text>
        {!locked && (data?.custom.length ?? 0) > 1 && (
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel="Mélanger les questions à venir"
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
      {/* Deleting one closes the gap and shuffling slides every card to its new place. */}
      {data?.custom.map((q) => (
        <Animated.View key={q.id} entering={isNew(q.id) ? motion.enter : undefined} exiting={motion.exit} layout={motion.layout}>
        <Pressable
          delayLongPress={400}
          onLongPress={q.author_id != null && q.author_id !== me?.id ? () => setToReport({ kind: 'question', id: q.id }) : undefined}>
          <Card
            style={{
              flexDirection: 'row',
              gap: 12,
              alignItems: 'center',
              padding: 14,
            }}>
            {q.author_id != null ? <Avatar member={byId.get(q.author_id)} size={32} /> : <MessageCircleQuestion size={20} color={t.mut} />}
            <QuestionText
              text={q.text}
              rank={q.kind === 'rank'}
              containerStyle={{ flex: 1 }}
              style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}
            />
            {/* Its author or the room owner can take it out before it's asked. */}
            {(q.author_id === me?.id || group?.my_role === 'owner') && (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Supprimer la question"
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
        </Pressable>
        </Animated.View>
      ))}
      <ModerationDialog target={toReport} onClose={() => setToReport(null)} />
      <ConfirmModal
        visible={!!toDelete}
        title="Supprimer cette question ?"
        message={toDelete ? `« ${toDelete.text.replace(/ ([?!:;»])/g, '\u00A0$1')} »\nElle ne sera jamais posée.` : undefined}
        confirmLabel="Supprimer"
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

/** One of the two "many at once" buttons under the composer. */
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
      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{label}</Text>
    </PressableScale>
  );
}
