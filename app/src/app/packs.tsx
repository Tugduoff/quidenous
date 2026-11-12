import { useMutation, useQueryClient } from '@tanstack/react-query';
import * as DocumentPicker from 'expo-document-picker';
import { router } from 'expo-router';
import { ChevronLeft, Download, Search } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Card, Display, Toggle } from '@/components/ui';
import { api, ApiError } from '@/lib/api';
import { usePacks } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const ERRORS: Record<string, string> = {
  bad_pack: 'Un nom (40 caractères max) et une description courte',
  pack_size: 'Il faut entre 5 et 300 questions',
  too_many_packs: 'Tu as déjà publié 10 packs',
};

/** Community packs: browse and search; publish one from this room's questions or a file. */
export default function PacksScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const [q, setQ] = useState('');
  const { data: packs } = usePacks(q.trim());
  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');
  const descField = useRef<ComponentRef<typeof TextInput>>(null);
  const [fromFile, setFromFile] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const publish = useMutation({
    mutationFn: async () => {
      let text: string | undefined;
      if (fromFile) {
        const picked = await DocumentPicker.getDocumentAsync({ type: ['text/plain', 'text/*'], copyToCacheDirectory: true });
        if (picked.canceled || !picked.assets[0]) throw new Error('cancel');
        const a = picked.assets[0];
        text = a.file ? await a.file.text() : await (await fetch(a.uri)).text();
      }
      return api<{ id: number; count: number }>('/packs', { method: 'POST', json: { name: name.trim(), description: desc.trim(), text } });
    },
    onSuccess: (r) => {
      setName('');
      setDesc('');
      setNote(`Publié : ${r.count} questions`);
      qc.invalidateQueries({ queryKey: ['packs'] });
    },
    onError: (e) =>
      setNote(
        e instanceof ApiError
          ? (ERRORS[e.code] ?? 'Raté, réessaie')
          : e instanceof Error && e.message === 'cancel'
            ? null
            : 'Raté, réessaie',
      ),
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

  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/questions'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>Packs</Display>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, paddingHorizontal: 14 }}>
        <Search size={18} color={t.mut} />
        <TextInput
          accessibilityLabel="Chercher un pack"
          placeholder="Chercher un pack"
          placeholderTextColor={t.mut}
          value={q}
          onChangeText={setQ}
          returnKeyType="search"
          style={{ flex: 1, height: 44, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
        />
      </Card>
      {packs?.length === 0 && <Text style={{ fontFamily: t.font.regular, color: t.mut, marginLeft: 4 }}>Aucun pack pour l’instant.</Text>}
      {packs?.map((p) => (
        <Pressable key={p.id} accessibilityRole="button" onPress={() => router.push(`/pack/${p.id}`)}>
          <Card style={{ padding: 16, gap: 6 }}>
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
              <Text style={{ flex: 1, fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>{p.name}</Text>
              <Download size={14} color={t.mut} />
              <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{p.installs}</Text>
            </View>
            <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
              {`${p.count} questions · par ${p.author}${p.description ? ` · ${p.description}` : ''}`}
            </Text>
            {p.sample.map((s) => (
              <Text key={s} numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.txt, opacity: 0.8 }}>
                {`· ${s}`}
              </Text>
            ))}
          </Card>
        </Pressable>
      ))}

      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4, marginTop: 8 }}>Publier un pack</Text>
      <Card style={{ padding: 16, gap: 10 }}>
        <TextInput
          accessibilityLabel="Nom du pack"
          placeholder="Nom du pack"
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
          accessibilityLabel="Description"
          placeholder="Description (optionnel)"
          placeholderTextColor={t.mut}
          value={desc}
          ref={descField}
          onChangeText={setDesc}
          maxLength={140}
          returnKeyType="send"
          onSubmitEditing={() => name.trim() && !publish.isPending && publish.mutate()}
          style={input}
        />
        <Toggle label="Depuis un fichier" value={fromFile} onChange={setFromFile} />
        <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>
          {fromFile ? 'Tu choisis le fichier en publiant.' : 'Sinon, les questions ajoutées dans ce salon.'}
        </Text>
        <Pressable
          accessibilityRole="button"
          disabled={!name.trim() || publish.isPending}
          onPress={() => publish.mutate()}
          style={{
            height: 46,
            borderRadius: t.radius.pill,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: name.trim() ? t.acc : t.card2,
          }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: name.trim() ? t.accTxt : t.mut }}>
            {publish.isPending ? 'Publication…' : 'Publier'}
          </Text>
        </Pressable>
        {note && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>{note}</Text>}
      </Card>
    </Screen>
  );
}
