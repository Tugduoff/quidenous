import { useMutation, useQueryClient } from '@tanstack/react-query';
import * as DocumentPicker from 'expo-document-picker';
import { router } from 'expo-router';
import { Check, FileUp, Lock, MessageCircleQuestion, Plus, Shuffle, Trash2, X } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, Card, ConfirmModal, Display, Toggle } from '@/components/ui';
import { api } from '@/lib/api';
import { parseQuestions, splitTag, type DraftLine } from '@/lib/import-draft';
import { keys, useGroup, useMembers, useQueue } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

const MIN = 8;

export default function QuestionsScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data } = useQueue();
  const { data: group } = useGroup();
  const { byId } = useMembers();
  const [text, setText] = useState('');
  const [rank, setRank] = useState(false);
  const { me } = useSession();
  const [toDelete, setToDelete] = useState<{ id: number; text: string } | null>(null);
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
  const pick = async () => {
    const picked = await DocumentPicker.getDocumentAsync({
      type: ['text/plain', 'text/*'],
      copyToCacheDirectory: true,
    });
    if (picked.canceled || !picked.assets[0]) return;
    const asset = picked.assets[0];
    const content = asset.file ? await asset.file.text() : await (await fetch(asset.uri)).text();
    const lines = parseQuestions(content);
    if (lines.length) {
      setMix(false);
      setDraft(lines);
    } else setImported('Fichier vide');
  };
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
      setImported('Mélangé 🔀');
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
            Dans ce salon, seul le propriétaire ajoute des questions.
          </Text>
        </Card>
      ) : (
        <>
          {/* Write one: the ranking switch and the add button sit under the text, in the same card. */}
          <Card style={{ paddingBottom: 10 }}>
            <TextInput
              accessibilityLabel="Nouvelle question"
              placeholder={rank ? 'Classe-nous du plus… au moins…' : 'Qui de nous…'}
              placeholderTextColor={t.mut}
              value={text}
              onChangeText={setText}
              multiline
              maxLength={200}
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
              <Toggle label="🏆 Classement" value={rank} onChange={setRank} />
              <View style={{ flex: 1 }} />
              <Pressable
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
              </Pressable>
            </View>
          </Card>
          {/* Or many at once. */}
          <View style={{ flexDirection: 'row', gap: 10 }}>
            <BulkButton label="Packs" emoji="📦" onPress={() => router.push('/packs')} />
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
            <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>
              {chosen.length} / {draft.length} à importer
            </Text>
            <Pressable accessibilityRole="button" onPress={toggleAll} hitSlop={8}>
              <Text
                style={{
                  fontFamily: t.font.semibold,
                  fontSize: 14,
                  color: t.accInk,
                }}>
                {allOn ? 'Tout décocher' : 'Tout cocher'}
              </Text>
            </Pressable>
          </View>
          {/* Ask them in a random order instead of the file's. */}
          <Toggle label="🔀 Ordre aléatoire" value={mix} onChange={setMix} />
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
              <Text
                style={{
                  flex: 1,
                  fontFamily: t.font.regular,
                  fontSize: 14,
                  lineHeight: 20,
                  color: t.txt,
                }}>
                {l.rank && <Text>🏆 </Text>}
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
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Mélanger les questions à venir"
            disabled={shuffle.isPending}
            onPress={() => shuffle.mutate()}
            hitSlop={8}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 6,
              height: 32,
              paddingHorizontal: 12,
              borderRadius: t.radius.pill,
              backgroundColor: t.accSoft,
              opacity: shuffle.isPending ? 0.5 : 1,
            }}>
            <Shuffle size={14} color={t.accInk} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.accInk }}>Mélanger</Text>
          </Pressable>
        )}
      </View>
      {data?.custom.map((q) => (
        <Card
          key={q.id}
          style={{
            flexDirection: 'row',
            gap: 12,
            alignItems: 'center',
            padding: 14,
          }}>
          {q.author_id != null ? <Avatar member={byId.get(q.author_id)} size={32} /> : <MessageCircleQuestion size={20} color={t.mut} />}
          <Text
            style={{
              flex: 1,
              fontFamily: t.font.semibold,
              fontSize: 15,
              color: t.txt,
            }}>
            {q.kind === 'rank' ? '🏆 ' : ''}
            {q.text}
          </Text>
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
      ))}
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
  emoji,
  icon,
  onPress,
  disabled,
  accessibilityLabel,
}: {
  label: string;
  emoji?: string;
  icon?: React.ReactNode;
  onPress: () => void;
  disabled?: boolean;
  accessibilityLabel?: string;
}) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={accessibilityLabel ?? label}
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => ({
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
        opacity: disabled ? 0.5 : pressed ? 0.7 : 1,
      })}>
      {emoji ? <Text style={{ fontSize: 15 }}>{emoji}</Text> : icon}
      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{label}</Text>
    </Pressable>
  );
}
