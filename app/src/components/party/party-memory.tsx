import { useMutation } from '@tanstack/react-query';
import { router } from 'expo-router';
import { ChevronRight } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { PartyHeader } from '@/components/party/party-header';
import { Avatar } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Screen } from '@/components/ui/screen';
import { SoftButton } from '@/components/ui/soft-button';
import { Display, ErrorText } from '@/components/ui/text';
import type { Member } from '@/lib/api/group';
import { type Party, partyApi, partyError, usePartyMemory } from '@/lib/api/party';
import { askConfirm } from '@/lib/confirm';
import { usePartyPeople } from '@/lib/hooks/use-party-people';
import { useSwitchRoom } from '@/lib/hooks/use-switch-room';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { tidy } from '@/lib/utils/text';
import { isPartyOwner } from '@/lib/utils/party';

export function PartyMemory({ party }: { party: Party }) {
  const t = useTheme();
  const tr = useT();
  const memory = usePartyMemory(party.id);
  const switchRoom = useSwitchRoom();
  const [error, setError] = useState<string | null>(null);
  const byId = usePartyPeople(party);
  const owner = isPartyOwner(party);
  const keep = useMutation({
    meta: { quiet: true },
    mutationFn: () => partyApi.keep(party.id),
    onMutate: () => setError(null),
    onSuccess: (r) => switchRoom(r?.id ?? party.id),
    onError: (e) => setError(partyError(e)),
  });
  const askKeep = async () => {
    if (await askConfirm(tr('party.memory.keepAsk'), tr('party.memory.keepMessage'), tr('common.continue'))) keep.mutate();
  };
  const guest = !!party.me?.guest;
  return (
    <Screen
      tabs={false}
      footer={
        (owner || guest) && (
          <>
            <ErrorText>{error}</ErrorText>
            {owner && (
              <Button loading={keep.isPending} onPress={askKeep}>
                {tr('party.memory.keep')}
              </Button>
            )}
            {guest &&
              (owner ? (
                <SoftButton onPress={() => router.push('/security')}>{tr('party.memory.signUp')}</SoftButton>
              ) : (
                <Button onPress={() => router.push('/security')}>{tr('party.memory.signUp')}</Button>
              ))}
          </>
        )
      }>
      <PartyHeader party={party} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <View style={{ flex: 1, gap: 6 }}>
          <Display size={34}>{tr('party.over')}</Display>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{tr('party.memory.played', { count: party.played })}</Text>
        </View>
        <Mascot who="paillette" mood="celebrate" size={104} />
      </View>
      {memory.isLoading && <ActivityIndicator color={t.mut} style={{ marginTop: 16 }} />}
      {memory.data?.map((h, i) => {
        const winners = h.winner_ids.map((wid) => byId.get(wid)).filter((m): m is Member => !!m);
        return (
          <Animated.View key={h.id} entering={motion.stagger(i)}>
            <Pressable accessibilityRole="button" onPress={() => router.push(`/round/${h.id}`)}>
              <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 14 }}>
                <View style={{ flex: 1, gap: 8 }}>
                  <Text numberOfLines={3} style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}>
                    {tidy(h.question)}
                  </Text>
                  {winners.length > 0 && (
                    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                      <Avatar member={winners[0]} size={24} />
                      <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 14, color: t.accInk }}>
                        {winners.map((m) => m.name).join(' · ')}
                      </Text>
                    </View>
                  )}
                </View>
                <ChevronRight size={18} color={t.mut} />
              </Card>
            </Pressable>
          </Animated.View>
        );
      })}
    </Screen>
  );
}
