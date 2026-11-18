import { useMutation } from '@tanstack/react-query';
import { Lock, LockOpen, LogOut, Square } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { PlayerChips } from '@/components/party/party';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import { Segmented } from '@/components/ui/segmented';
import { ErrorText } from '@/components/ui/text';
import { ApiError } from '@/lib/api/client';
import { useFeatures } from '@/lib/api/features';
import { type Party, partyApi, partyError, TIMERS, useSetParty } from '@/lib/api/party';
import { askConfirm } from '@/lib/confirm';
import { useRemovePlayer } from '@/lib/hooks/use-remove-player';
import { useRoomGone } from '@/lib/hooks/use-switch-room';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { showToast } from '@/lib/toast';
import { isPartyOwner } from '@/lib/utils/party';

export function PartyMenu({ party, visible, onClose }: { party: Party; visible: boolean; onClose: () => void }) {
  const t = useTheme();
  const tr = useT();
  const setParty = useSetParty(party.id);
  const roomGone = useRoomGone();
  const [error, setError] = useState<string | null>(null);
  const owner = isPartyOwner(party);
  const live = party.phase !== 'over';
  const timer = useMutation({
    meta: { quiet: true },
    mutationFn: (s: number) => partyApi.update(party.id, { timer: s }),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => setError(partyError(e)),
  });
  const lock = useMutation({
    meta: { quiet: true },
    mutationFn: (locked: boolean) => partyApi.update(party.id, { locked }),
    onMutate: () => setError(null),
    onSuccess: setParty,
    onError: (e) => setError(partyError(e)),
  });
  const removePlayer = useRemovePlayer(party.id);
  const { moderation } = useFeatures();
  const others = party.players.filter((p) => p.role !== 'owner');
  const end = async () => {
    onClose();
    if (!(await askConfirm(tr('party.menu.endAsk'), tr('party.menu.endMessage'), tr('party.menu.endAction')))) return;
    partyApi.end(party.id).then(setParty, (e) => showToast(partyError(e)));
  };
  const leave = async () => {
    onClose();
    if (!(await askConfirm(tr('party.menu.leaveAsk'), tr('party.menu.leaveMessage'), tr('common.leave')))) return;
    try {
      await partyApi.leave(party.id);
    } catch (e) {
      const out = e instanceof ApiError && (e.status === 404 || e.code === 'not_in_room' || e.code === 'removed');
      if (!out) return showToast(partyError(e));
    }
    await roomGone();
  };
  const row = (Icon: typeof LogOut, label: string, onPress: () => void, danger?: boolean) => (
    <Pressable
      accessibilityRole="button"
      onPress={onPress}
      style={{ flexDirection: 'row', alignItems: 'center', gap: 12, height: 48, paddingHorizontal: 14, borderRadius: 14, backgroundColor: t.card2 }}>
      <Icon size={18} color={danger ? t.tagInk : t.txt} />
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: danger ? t.tagInk : t.txt }}>{label}</Text>
    </Pressable>
  );
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title={party.name || tr('party.name')} />
      {owner && live && (
        <View style={{ gap: 8 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('party.menu.timer')}</Text>
          <Segmented
            label={tr('party.menu.timer')}
            value={String(party.timer)}
            onChange={(v) => timer.mutate(Number(v))}
            options={TIMERS.map((s) => ({ id: String(s), name: tr('party.create.seconds', { count: s }) }))}
          />
        </View>
      )}
      {owner && live && moderation && others.length > 0 && (
        <View style={{ gap: 8 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('party.menu.players')}</Text>
          <PlayerChips
            players={others}
            onPress={(p) => {
              onClose();
              removePlayer(p);
            }}
          />
        </View>
      )}
      <ErrorText>{error}</ErrorText>
      {owner &&
        live &&
        moderation &&
        (party.locked ? row(LockOpen, tr('party.menu.unlock'), () => lock.mutate(false)) : row(Lock, tr('party.menu.lock'), () => lock.mutate(true)))}
      {owner && live && row(Square, tr('party.menu.end'), end, true)}
      {row(LogOut, tr('party.menu.leave'), leave)}
    </Dialog>
  );
}
