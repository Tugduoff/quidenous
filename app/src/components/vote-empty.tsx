import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Check, ChevronRight, FileUp, Lock, type LucideIcon, Package, PenLine } from 'lucide-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { AddFriends, InviteCard } from '@/components/invite';
import { Button, Card, Display, PressableScale } from '@/components/ui';
import { api, ApiError } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { handOffDraft, pickQuestionFile } from '@/lib/import-draft';
import { motion } from '@/lib/motion';
import { keys, useGroup } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

/** Alone in the room: nothing to vote on until someone joins, so the whole tab is the invite. */
export function SoloInvite() {
  const t = useTheme();
  return (
    <Animated.View entering={motion.enter} style={{ gap: 16 }}>
      <View style={{ gap: 6 }}>
        <Display size={30}>Invite tes amis</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>
          La première question arrive dès que quelqu'un te rejoint.
        </Text>
      </View>
      <InviteCard qrSize={208} rotate={false} />
      <AddFriends onlyIfAny />
    </Animated.View>
  );
}

function Action({ icon: Icon, label, onPress }: { icon: LucideIcon; label: string; onPress: () => void }) {
  const t = useTheme();
  return (
    <PressableScale scale={0.98} accessibilityRole="button" onPress={onPress}>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 14, padding: 14 }}>
        <View style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.accSoft }}>
          <Icon size={20} color={t.accInk} />
        </View>
        <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>{label}</Text>
        <ChevronRight size={18} color={t.mut} />
      </Card>
    </PressableScale>
  );
}

/** Rooms I already asked the owner in, this session (the server says so too, with `already_asked`). */
const asked = new Set<number>();

/** The queue is empty: ways to add questions, or (owner-only room) ask the owner for some. */
export function NoQuestions() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const features = useFeatures();
  const [note, setNote] = useState<string | null>(null);
  const [done, setDone] = useState(() => group != null && asked.has(group.id));
  const markAsked = () => {
    if (group) asked.add(group.id);
    setDone(true);
  };
  const ask = useMutation({
    mutationFn: () => api('/questions/ask', { method: 'POST' }),
    onSuccess: () => markAsked(),
    onError: (e) => {
      if (e instanceof ApiError && e.code === 'already_asked') return markAsked();
      // The owner opened questions to everyone meanwhile: the actions show once the room is fresh.
      if (e instanceof ApiError && e.code === 'not_needed') return qc.invalidateQueries({ queryKey: keys.group });
      setNote('Raté, réessaie');
    },
  });
  const locked = group?.settings.questions_by === 'owner' && group.my_role !== 'owner';
  // Picked right in the tap (browsers want that), then reviewed on the Questions tab.
  const importFile = async () => {
    setNote(null);
    const lines = await pickQuestionFile().catch(() => null);
    if (lines === 'empty') return setNote('Fichier vide');
    if (!lines) return;
    handOffDraft(lines);
    router.navigate({ pathname: '/questions', params: { import: '1' } });
  };

  return (
    <Animated.View entering={motion.enter} style={{ gap: 16 }}>
      <Display size={30}>Plus de questions</Display>
      {locked ? (
        <>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            <Lock size={18} color={t.mut} />
            <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>
              Ici, c'est le propriétaire qui en ajoute.
            </Text>
          </Card>
          {features.askQuestions &&
            (done ? (
              <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 8, height: 58 }}>
                <Check size={20} color={t.accInk} />
                <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.accInk }}>Demande envoyée</Text>
              </View>
            ) : (
              <Button loading={ask.isPending} onPress={() => ask.mutate()}>
                Demander au propriétaire
              </Button>
            ))}
        </>
      ) : (
        <View style={{ gap: 10 }}>
          <Action icon={PenLine} label="Écrire une question" onPress={() => router.navigate({ pathname: '/questions', params: { compose: '1' } })} />
          <Action icon={Package} label="Installer un pack" onPress={() => router.push('/packs')} />
          <Action icon={FileUp} label="Importer un fichier" onPress={importFile} />
        </View>
      )}
      {note && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag, marginLeft: 4 }}>{note}</Text>}
    </Animated.View>
  );
}
