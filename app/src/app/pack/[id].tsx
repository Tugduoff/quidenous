import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { ChevronLeft, Flag, Trash2 } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Card, Display, PressableScale, QuestionText } from '@/components/ui';
import { api, ApiError } from '@/lib/api';
import { keys, useGroup, usePack } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const round = (bg: string) => ({ width: 40, height: 40, borderRadius: 20, alignItems: 'center' as const, justifyContent: 'center' as const, backgroundColor: bg });

/** One pack: all its questions, install into the current room, report, delete (author). */
export default function PackScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const id = Number(useLocalSearchParams<{ id: string }>().id);
  const { data: pack } = usePack(id);
  const { data: group } = useGroup();
  const [note, setNote] = useState<string | null>(null);
  const install = useMutation({
    mutationFn: () => api<{ added: number; skipped: number }>(`/packs/${id}/install`, { method: 'POST' }),
    onSuccess: (r) => {
      setNote(`+${r.added} questions dans « ${group?.name} »${r.skipped ? ` (${r.skipped} déjà là)` : ''}`);
      qc.invalidateQueries({ queryKey: keys.queue });
      qc.invalidateQueries({ queryKey: keys.current });
    },
    onError: (e) =>
      setNote(e instanceof ApiError && e.code === 'owner_only' ? 'Réservé au propriétaire du salon' : 'Raté, réessaie'),
  });
  const report = useMutation({
    mutationFn: () => api(`/packs/${id}/report`, { method: 'POST' }),
    onSuccess: () => setNote('Merci, c’est signalé'),
  });
  const remove = useMutation({
    mutationFn: () => api(`/packs/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['packs'] });
      router.back();
    },
  });
  if (!pack) return <Screen tabs={false}>{null}</Screen>;
  return (
    <Screen
      tabs={false}
      footer={
        <Pressable
          accessibilityRole="button"
          disabled={install.isPending}
          onPress={() => install.mutate()}
          style={{ height: 52, paddingHorizontal: 20, borderRadius: t.radius.pill, alignItems: 'center', justifyContent: 'center', backgroundColor: t.acc }}>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 16, color: t.accTxt }}>{`Installer dans « ${group?.name ?? '…'} »`}</Text>
        </Pressable>
      }>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/packs'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <View style={{ gap: 4 }}>
        <Display size={28}>{pack.name}</Display>
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
          {`${pack.questions.length} questions · par ${pack.author} · ${pack.installs} installations`}
        </Text>
        {!!pack.description && <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.txt }}>{pack.description}</Text>}
      </View>
      {note && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>{note}</Text>}
      <Card style={{ padding: 16, gap: 10 }}>
        {pack.questions.map((q, i) => (
          <QuestionText
            key={i}
            text={q.text}
            rank={q.kind === 'rank'}
            style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 20, color: t.txt }}
          />
        ))}
      </Card>
      <View style={{ flexDirection: 'row', justifyContent: 'center', gap: 20 }}>
        {pack.mine ? (
          <PressableScale accessibilityRole="button" accessibilityLabel="Retirer mon pack" onPress={() => remove.mutate()} style={round(t.card)}>
            <Trash2 size={18} color={t.tag} />
          </PressableScale>
        ) : (
          <PressableScale accessibilityRole="button" accessibilityLabel="Signaler" onPress={() => report.mutate()} style={round(t.card)}>
            <Flag size={18} color={t.mut} />
          </PressableScale>
        )}
      </View>
    </Screen>
  );
}
