import { useMutation, useQueryClient } from '@tanstack/react-query';
import * as DocumentPicker from 'expo-document-picker';
import { Check, FileUp, MessageCircleQuestion, Plus, X } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, Card, Display } from '@/components/ui';
import { api } from '@/lib/api';
import { parseQuestions, type DraftLine } from '@/lib/import-draft';
import { keys, useMembers, useQueue } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const MIN = 8;

export default function QuestionsScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data } = useQueue();
  const { byId } = useMembers();
  const [text, setText] = useState('');
  const add = useMutation({
    mutationFn: () => api('/questions', { method: 'POST', json: { text } }),
    onSuccess: () => {
      setText('');
      qc.invalidateQueries({ queryKey: keys.queue });
      qc.invalidateQueries({ queryKey: keys.current });
    },
  });
  const ok = text.trim().length >= MIN && !add.isPending;
  const [imported, setImported] = useState<string | null>(null);
  // Pick a file, show what's in it, send only what's ticked.
  const [draft, setDraft] = useState<DraftLine[] | null>(null);
  const pick = async () => {
    const picked = await DocumentPicker.getDocumentAsync({ type: ['text/plain', 'text/*'], copyToCacheDirectory: true });
    if (picked.canceled || !picked.assets[0]) return;
    const asset = picked.assets[0];
    const content = asset.file ? await asset.file.text() : await (await fetch(asset.uri)).text();
    const lines = parseQuestions(content);
    if (lines.length) setDraft(lines);
    else setImported('Fichier vide');
  };
  const importFile = useMutation({
    mutationFn: (lines: string[]) =>
      api<{ added: number; skipped: number }>('/questions/import', { method: 'POST', json: { text: lines.join('\n') } }),
    onSuccess: (res) => {
      setDraft(null);
      setImported(`+${res.added}` + (res.skipped ? ` · ${res.skipped} déjà là` : ''));
      qc.invalidateQueries({ queryKey: keys.queue });
      qc.invalidateQueries({ queryKey: keys.current });
    },
    onError: () => setImported('Import raté'),
  });
  const chosen = draft?.filter((l) => l.selected).map((l) => l.text) ?? [];
  const toggle = (i: number) =>
    setDraft((d) => d && d.map((l, j) => (j === i && !l.problem ? { ...l, selected: !l.selected } : l)));
  const allOn = !!draft && draft.every((l) => l.selected || l.problem);
  const toggleAll = () => setDraft((d) => d && d.map((l) => (l.problem ? l : { ...l, selected: !allOn })));

  return (
    <Screen>
      <Display size={30}>Questions</Display>
      <View style={{ flexDirection: 'row', gap: 10, alignItems: 'flex-end' }}>
        <Card style={{ flex: 1 }}>
          <TextInput
            accessibilityLabel="Nouvelle question"
            placeholder="Qui de nous…"
            placeholderTextColor={t.mut}
            value={text}
            onChangeText={setText}
            multiline
            maxLength={200}
            style={{ minHeight: 96, padding: 16, fontFamily: t.font.regular, fontSize: 16, color: t.txt, textAlignVertical: 'top' }}
          />
        </Card>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Ajouter"
          disabled={!ok}
          onPress={() => add.mutate()}
          style={{ width: 52, height: 52, borderRadius: 26, backgroundColor: ok ? t.acc : t.card2, alignItems: 'center', justifyContent: 'center' }}>
          <Plus size={26} color={ok ? t.accTxt : t.mut} />
        </Pressable>
      </View>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
        <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {(data?.custom.length ?? 0) + (data?.deck_left ?? 0)}
          {imported ? `  ·  ${imported}` : ''}
        </Text>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Importer un fichier de questions"
          disabled={importFile.isPending}
          onPress={pick}
          style={[{ width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
          <FileUp size={20} color={importFile.isPending ? t.mut : t.txt} />
        </Pressable>
      </View>
      {draft && (
        <Card style={{ padding: 16, gap: 12 }}>
          <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
            <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>
              {chosen.length} / {draft.length} à importer
            </Text>
            <Pressable accessibilityRole="button" onPress={toggleAll} hitSlop={8}>
              <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>{allOn ? 'Tout décocher' : 'Tout cocher'}</Text>
            </Pressable>
          </View>
          {draft.map((l, i) => (
            <Pressable
              key={i}
              accessibilityRole="checkbox"
              accessibilityState={{ checked: l.selected, disabled: !!l.problem }}
              onPress={() => toggle(i)}
              style={{ flexDirection: 'row', gap: 10, alignItems: 'flex-start', opacity: l.problem ? 0.5 : 1 }}>
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
              <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 14, lineHeight: 20, color: t.txt }}>
                {l.text}
                {l.problem && <Text style={{ color: t.tag }}>{`  · ${l.problem}`}</Text>}
              </Text>
            </Pressable>
          ))}
          <View style={{ flexDirection: 'row', gap: 10 }}>
            <Pressable
              accessibilityRole="button"
              onPress={() => setDraft(null)}
              style={{ flex: 1, height: 48, borderRadius: t.radius.pill, alignItems: 'center', justifyContent: 'center', flexDirection: 'row', gap: 6, backgroundColor: t.card2 }}>
              <X size={18} color={t.txt} />
              <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Annuler</Text>
            </Pressable>
            <Pressable
              accessibilityRole="button"
              disabled={!chosen.length || importFile.isPending}
              onPress={() => importFile.mutate(chosen)}
              style={{ flex: 2, height: 48, borderRadius: t.radius.pill, alignItems: 'center', justifyContent: 'center', backgroundColor: chosen.length ? t.acc : t.card2 }}>
              <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: chosen.length ? t.accTxt : t.mut }}>
                {importFile.isPending ? 'Import…' : `Importer ${chosen.length}`}
              </Text>
            </Pressable>
          </View>
        </Card>
      )}
      {data?.custom.map((q) => (
        <Card key={q.id} style={{ flexDirection: 'row', gap: 12, alignItems: 'center', padding: 14 }}>
          {q.author_id != null ? (
            <Avatar member={byId.get(q.author_id)} size={32} />
          ) : (
            <MessageCircleQuestion size={20} color={t.mut} />
          )}
          <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{q.text}</Text>
        </Card>
      ))}
    </Screen>
  );
}
