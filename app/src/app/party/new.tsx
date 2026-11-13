import { useMutation } from '@tanstack/react-query';
import { router } from 'expo-router';
import { ChevronLeft, PartyPopper } from 'lucide-react-native';
import { useEffect, useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';

import { ErrorText } from '@/components/auth-ui';
import { Screen } from '@/components/screen';
import { Button, Card, Display, Segmented } from '@/components/ui';
import { VibeCards } from '@/components/vibes';
import { useFeatures } from '@/lib/auth';
import { getLang, useT } from '@/lib/i18n';
import { HOURS, partyApi, partyError, TIMERS } from '@/lib/party';
import { useVibes } from '@/lib/queries';
import { useSwitchRoom } from '@/lib/rooms';
import { useTheme } from '@/lib/theme';

/** `/party/new`: name (optional), vibe, how long the party lasts and the time per question. Then its lobby. */
export default function NewParty() {
  const t = useTheme();
  const tr = useT();
  const features = useFeatures();
  const { data: vibes } = useVibes(features.vibes);
  const switchRoom = useSwitchRoom();
  const [name, setName] = useState('');
  const [vibe, setVibe] = useState<string | null>(null);
  const [hours, setHours] = useState('4');
  const [timer, setTimer] = useState('30');
  const [error, setError] = useState<string | null>(null);
  // The usual tone until another one is picked.
  useEffect(() => {
    if (vibes?.length && vibe == null) setVibe(vibes.find((v) => v.id === 'potes')?.id ?? vibes[0].id);
  }, [vibes, vibe]);

  const create = useMutation({
    mutationFn: () =>
      partyApi.create({
        ...(name.trim() ? { name: name.trim() } : {}),
        ...(vibe ? { vibe } : {}),
        ...(features.languages ? { language: getLang() } : {}),
        hours: Number(hours),
        timer: Number(timer),
      }),
    onMutate: () => setError(null),
    onSuccess: (r) => switchRoom(r.id, 'party'),
    onError: (e) => setError(partyError(e)),
  });

  const label = { fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 };
  const choice = (title: string, value: string, onChange: (v: string) => void, options: { id: string; name: string }[]) => (
    <View style={{ gap: 8, alignItems: 'flex-start' }}>
      <Text numberOfLines={1} style={label}>
        {title}
      </Text>
      <Segmented label={title} value={value} onChange={onChange} options={options} />
    </View>
  );

  return (
    <Screen
      tabs={false}
      footer={
        <>
          <ErrorText>{error}</ErrorText>
          <Button loading={create.isPending} onPress={() => create.mutate()}>
            <PartyPopper size={20} color={t.accTxt} />
            <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{tr('party.create.go')}</Text>
          </Button>
        </>
      }>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('common.back')}
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/rooms'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>{tr('party.create.title')}</Display>

      <Card style={{ paddingHorizontal: 16 }}>
        <TextInput
          accessibilityLabel={tr('party.create.name')}
          placeholder={tr('party.create.placeholder')}
          placeholderTextColor={t.mut}
          value={name}
          onChangeText={setName}
          maxLength={40}
          returnKeyType="done"
          style={{ height: 52, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
        />
      </Card>

      {choice(
        tr('party.create.duration'),
        hours,
        setHours,
        HOURS.map((h) => ({ id: String(h), name: tr('party.create.hours', { count: h }) })),
      )}
      {choice(
        tr('party.create.timer'),
        timer,
        setTimer,
        TIMERS.map((s) => ({ id: String(s), name: tr('party.create.seconds', { count: s }) })),
      )}

      {!!vibes?.length && (
        <View style={{ gap: 10 }}>
          <Text style={label}>{tr('party.create.vibe')}</Text>
          <VibeCards vibes={vibes} value={vibe} onChange={setVibe} />
        </View>
      )}
    </Screen>
  );
}
