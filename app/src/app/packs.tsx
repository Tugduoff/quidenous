import { useMutation, useQueryClient } from '@tanstack/react-query';
import * as DocumentPicker from 'expo-document-picker';
import { router } from 'expo-router';
import { ChevronLeft, Download, Search } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Card, Display, Toggle } from '@/components/ui';
import { api, ApiError } from '@/lib/api';
import { getLang, type TKey, useT } from '@/lib/i18n';
import { useGroup, usePacks } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const ERRORS: Record<string, TKey> = {
  bad_pack: 'packs.errors.bad_pack',
  pack_size: 'packs.errors.pack_size',
  too_many_packs: 'packs.errors.too_many_packs',
};

/** Community packs: browse and search; publish one from this room's questions or a file. */
export default function PacksScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const [q, setQ] = useState('');
  const { data: packs } = usePacks(q.trim());
  // A pack in another language than the room's gets a small tag (no tag when either is unknown: older server).
  const { data: group } = useGroup();
  const roomLang = group?.language;
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
      // The pack's language: the room's when known, else the app's.
      const language = group?.language ?? getLang();
      return api<{ id: number; count: number }>('/packs', { method: 'POST', json: { name: name.trim(), description: desc.trim(), text, language } });
    },
    onSuccess: (r) => {
      setName('');
      setDesc('');
      setNote(tr('packs.published', { count: r.count }));
      qc.invalidateQueries({ queryKey: ['packs'] });
    },
    onError: (e) =>
      setNote(
        e instanceof ApiError
          ? tr(ERRORS[e.code] ?? 'common.failed')
          : e instanceof Error && e.message === 'cancel'
            ? null
            : tr('common.failed'),
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
        accessibilityLabel={tr('common.back')}
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/questions'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>{tr('packs.title')}</Display>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, paddingHorizontal: 14 }}>
        <Search size={18} color={t.mut} />
        <TextInput
          accessibilityLabel={tr('packs.search')}
          placeholder={tr('packs.search')}
          placeholderTextColor={t.mut}
          value={q}
          onChangeText={setQ}
          returnKeyType="search"
          style={{ flex: 1, minWidth: 0, height: 44, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
        />
      </Card>
      {packs?.length === 0 && <Text style={{ fontFamily: t.font.regular, color: t.mut, marginLeft: 4 }}>{tr('packs.none')}</Text>}
      {packs?.map((p) => (
        <Pressable key={p.id} accessibilityRole="button" onPress={() => router.push(`/pack/${p.id}`)}>
          <Card style={{ padding: 16, gap: 6 }}>
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
              <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
                {p.name}
              </Text>
              {!!p.language && !!roomLang && p.language !== roomLang && <LangTag lang={p.language} />}
              <Download size={14} color={t.mut} />
              <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{p.installs}</Text>
            </View>
            <Text numberOfLines={2} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
              {tr('packs.byline', { count: p.count, author: p.author }) + (p.description ? ` · ${p.description}` : '')}
            </Text>
            {p.sample.map((s) => (
              <Text key={s} numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.txt, opacity: 0.8 }}>
                {`· ${s}`}
              </Text>
            ))}
          </Card>
        </Pressable>
      ))}

      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4, marginTop: 8 }}>{tr('packs.publishTitle')}</Text>
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
          accessibilityLabel={tr('packs.description')}
          placeholder={tr('packs.descriptionPlaceholder')}
          placeholderTextColor={t.mut}
          value={desc}
          ref={descField}
          onChangeText={setDesc}
          maxLength={140}
          returnKeyType="send"
          onSubmitEditing={() => name.trim() && !publish.isPending && publish.mutate()}
          style={input}
        />
        <Toggle label={tr('packs.fromFile')} value={fromFile} onChange={setFromFile} />
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
            {publish.isPending ? tr('packs.publishing') : tr('packs.publish')}
          </Text>
        </Pressable>
        {note && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>{note}</Text>}
      </Card>
    </Screen>
  );
}

/** A pack in another language than the room's: "EN", "FR". */
function LangTag({ lang }: { lang: string }) {
  const t = useTheme();
  return (
    <View style={{ paddingHorizontal: 6, height: 18, borderRadius: 6, justifyContent: 'center', backgroundColor: t.card2 }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: 10, letterSpacing: 0.5, color: t.mut }}>{lang.toUpperCase()}</Text>
    </View>
  );
}
