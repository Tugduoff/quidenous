import { useMutation } from '@tanstack/react-query';
import { Check, Copy, Share2, UserPlus } from 'lucide-react-native';
import { useState } from 'react';
import { Platform, Text, useWindowDimensions, View } from 'react-native';
import { JoinQr, PlayerChips } from '@/components/party/party';
import { PartyHeader } from '@/components/party/party-header';
import { ScreenLink } from '@/components/party/screen-link';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Screen } from '@/components/ui/screen';
import { CodeText, ErrorText } from '@/components/ui/text';
import { useFeatures } from '@/lib/api/features';
import { type Party, partyApi, partyError, useSetParty } from '@/lib/api/party';
import { copyText } from '@/lib/device/clipboard';
import { shareText } from '@/lib/device/share';
import { useRemovePlayer } from '@/lib/hooks/use-remove-player';
import { useT } from '@/lib/i18n';
import { partyJoinLink } from '@/lib/links';
import { useTheme } from '@/lib/theme';
import { isPartyOwner } from '@/lib/utils/party';

/** Invite, wait for everyone, and (owner) start. */
export function PartyLobby({ party }: { party: Party }) {
  const t = useTheme();
  const tr = useT();
  const { width } = useWindowDimensions();
  const setParty = useSetParty(party.id);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<'link' | 'code' | null>(null);
  const flash = (what: 'link' | 'code') => {
    setCopied(what);
    setTimeout(() => setCopied(null), 1800);
  };
  const start = useMutation({
    meta: { quiet: true },
    mutationFn: () => partyApi.start(party.id),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => setError(partyError(e)),
  });
  const owner = isPartyOwner(party);
  const removePlayer = useRemovePlayer(party.id);
  const { moderation } = useFeatures();
  const enough = party.players.length >= 2;
  const qr = Math.min(260, Math.max(160, Math.min(width, 520) - 120));
  const share = async () => {
    const r = await shareText(tr('party.lobby.message', { name: party.name || tr('party.name'), link: partyJoinLink(party.code) }));
    if (r === 'copied') flash('link');
  };
  const round = { width: 44, height: 44, borderRadius: 22, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 } as const;
  return (
    <Screen
      tabs={false}
      footer={
        <>
          <ErrorText>{error}</ErrorText>
          {owner && !enough && (
            <Text style={{ textAlign: 'center', fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('party.errors.needTwo')}</Text>
          )}
          {owner ? (
            <Button disabled={!enough} loading={start.isPending} onPress={() => start.mutate()}>
              {tr('party.lobby.start')}
            </Button>
          ) : (
            <Text style={{ textAlign: 'center', paddingVertical: 12, fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>
              {tr('party.lobby.waiting')}
            </Text>
          )}
        </>
      }>
      <PartyHeader party={party} />
      <Card style={{ padding: 18, gap: 14 }}>
        <JoinQr code={party.code} size={qr} />
        <CodeText code={party.code} size={48} center />
        <View style={{ flexDirection: 'row', gap: 10 }}>
          <PressableScale
            accessibilityRole="button"
            onPress={share}
            style={{
              flex: 1,
              height: 44,
              borderRadius: t.radius.pill,
              flexDirection: 'row',
              gap: 8,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: t.acc,
            }}>
            <Share2 size={18} color={t.accTxt} />
            <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 15, color: t.accTxt }}>
              {copied === 'link' ? tr('party.lobby.linkCopied') : tr('party.lobby.share')}
            </Text>
          </PressableScale>
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={copied === 'code' ? tr('room.invite.codeCopied') : tr('room.invite.copyCode')}
            onPress={async () => (await copyText(party.code)) && Platform.OS === 'web' && flash('code')}
            style={round}>
            {copied === 'code' ? <Check size={18} color={t.accInk} /> : <Copy size={18} color={t.txt} />}
          </PressableScale>
        </View>
      </Card>

      <View style={{ gap: 10 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, marginLeft: 4 }}>
          <UserPlus size={14} color={t.mut} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>
            {tr('party.lobby.players', { count: party.players.length })}
          </Text>
        </View>
        <PlayerChips players={party.players} onPress={owner && moderation ? removePlayer : undefined} />
      </View>

      <ScreenLink code={party.code} />
    </Screen>
  );
}
