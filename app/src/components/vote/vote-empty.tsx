import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Check, ChevronRight, FileUp, Lock, type LucideIcon, Package, PenLine } from 'lucide-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { AddFriends, InviteCard } from '@/components/rooms/invite';
import { Mascot } from '@/components/mascot/mascot';
import { ApiError } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Display } from '@/components/ui/text';
import { useGroup, groupKeys } from '@/lib/api/group';
import { askForQuestions } from '@/lib/api/questions';
import { useFeatures } from '@/lib/api/features';
import { pickQuestionFile, handOffDraft } from '@/lib/device/files';

export function SoloInvite() {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View entering={motion.enter} style={{ gap: 16 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <View style={{ flex: 1, gap: 6 }}>
          <Display size={30}>{tr('vote.solo.title')}</Display>
          <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{tr('vote.solo.body')}</Text>
        </View>
        <Mascot who="zeste" mood="happy" size={96} />
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
        <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
          {label}
        </Text>
        <ChevronRight size={18} color={t.mut} />
      </Card>
    </PressableScale>
  );
}

const asked = new Set<number>();

export function NoQuestions() {
  const t = useTheme();
  const tr = useT();
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
    meta: { quiet: true },
    mutationFn: () => askForQuestions(),
    onSuccess: () => markAsked(),
    onError: (e) => {
      if (e instanceof ApiError && e.code === 'already_asked') return markAsked();
      if (e instanceof ApiError && e.code === 'not_needed') return qc.invalidateQueries({ queryKey: groupKeys.all });
      setNote(tr('common.failed'));
    },
  });
  const locked = group?.settings.questions_by === 'owner' && group.my_role !== 'owner';
  const importFile = async () => {
    setNote(null);
    const lines = await pickQuestionFile().catch(() => null);
    if (lines === 'empty') return setNote(tr('vote.empty.emptyFile'));
    if (!lines) return;
    handOffDraft(lines);
    router.navigate({ pathname: '/questions', params: { import: '1' } });
  };

  return (
    <Animated.View entering={motion.enter} style={{ gap: 16 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <View style={{ flex: 1 }}>
          <Display size={30}>{tr('vote.empty.title')}</Display>
        </View>
        <Mascot who="pave" mood="think" size={96} />
      </View>
      {locked ? (
        <>
          <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
            <Lock size={18} color={t.mut} />
            <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{tr('vote.empty.ownerOnly')}</Text>
          </Card>
          {features.askQuestions &&
            (done ? (
              <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 8, height: 58 }}>
                <Check size={20} color={t.accInk} />
                <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.accInk }}>{tr('vote.empty.asked')}</Text>
              </View>
            ) : (
              <Button loading={ask.isPending} onPress={() => ask.mutate()}>
                {tr('vote.empty.ask')}
              </Button>
            ))}
        </>
      ) : (
        <View style={{ gap: 10 }}>
          <Action
            icon={PenLine}
            label={tr('vote.empty.write')}
            onPress={() => router.navigate({ pathname: '/questions', params: { compose: '1' } })}
          />
          <Action icon={Package} label={tr('vote.empty.pack')} onPress={() => router.push('/packs')} />
          <Action icon={FileUp} label={tr('vote.empty.import')} onPress={importFile} />
        </View>
      )}
      {note && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tagInk, marginLeft: 4 }}>{note}</Text>}
    </Animated.View>
  );
}
