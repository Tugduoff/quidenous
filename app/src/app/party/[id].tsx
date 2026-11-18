import { useLocalSearchParams } from 'expo-router';
import { ActivityIndicator, Text } from 'react-native';
import { BackToRooms, PartyHeader } from '@/components/party/party-header';
import { PartyLobby } from '@/components/party/party-lobby';
import { PartyMemory } from '@/components/party/party-memory';
import { PartyQuestion } from '@/components/party/party-question';
import { PartyReveal } from '@/components/party/party-reveal';
import { PartyVoting } from '@/components/party/party-voting';
import { PartySaidVoting, saidBoardKey } from '@/components/party/said';
import { Screen } from '@/components/ui/screen';
import { ApiError } from '@/lib/api/client';
import { partyError, useParty } from '@/lib/api/party';
import { useEnterPartyRoom } from '@/lib/hooks/use-party';
import { useRedirect } from '@/lib/hooks/use-redirect';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export default function PartyScreen() {
  const t = useTheme();
  const tr = useT();
  const { id: raw } = useLocalSearchParams<{ id: string }>();
  const id = Number(raw);
  const party = useParty(id);
  const p = party.data;
  const loaded = !!p;

  useEnterPartyRoom(id, loaded);

  const gone = party.error instanceof ApiError && [404, 409].includes(party.error.status);
  useRedirect(p?.kind === 'room' || (loaded && gone) ? '/' : null);

  if (!p)
    return (
      <Screen tabs={false}>
        <BackToRooms />
        {party.error ? (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.mut }}>{partyError(party.error)}</Text>
        ) : (
          <ActivityIndicator color={t.mut} style={{ marginTop: 40 }} />
        )}
      </Screen>
    );
  if (p.phase === 'lobby') return <PartyLobby party={p} />;
  const said = p.round?.kind === 'said' ? p.round.said : null;
  if (p.phase === 'voting' && p.round && said)
    return (
      <PartySaidVoting
        key={`${p.round.id}-${said.phase}-${saidBoardKey(said)}`}
        party={p}
        round={p.round}
        said={said}
        top={
          <>
            <PartyHeader party={p} />
            <PartyQuestion round={p.round} size={32} />
          </>
        }
      />
    );
  if (p.phase === 'voting' && p.round) return <PartyVoting key={p.round.id} party={p} round={p.round} />;
  if (p.phase === 'reveal' && p.round) return <PartyReveal key={p.round.id} party={p} round={p.round} />;
  if (p.phase === 'over') return <PartyMemory party={p} />;
  return (
    <Screen tabs={false}>
      <PartyHeader party={p} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.mut }}>{tr('common.failed')}</Text>
    </Screen>
  );
}
