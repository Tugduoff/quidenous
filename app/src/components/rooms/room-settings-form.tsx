import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Hourglass, MessageCircleQuestion, RefreshCw, Settings2, Ticket, TriangleAlert, Trash2, Vote } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator } from 'react-native';
import { AnonymousSetting } from '@/components/vote/anonymous';
import { showToast } from '@/lib/toast';
import { useT } from '@/lib/i18n';
import { useRoomGone } from '@/lib/hooks/use-switch-room';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { ErrorText } from '@/components/ui/text';
import { Toggle } from '@/components/ui/toggle';
import { Segmented } from '@/components/ui/segmented';
import { askConfirm } from '@/lib/confirm';
import { SettingsRow, SettingsSection } from '@/components/rooms/settings-section';
import { NameDialog } from '@/components/rooms/name-dialog';
import { DelayDialog, delayLabel } from '@/components/rooms/delay-dialog';
import { VibeRow } from '@/components/rooms/vibe-row';
import { HandOverDialog } from '@/components/rooms/hand-over-dialog';
import { type Group, groupKeys, type RoomPatch, updateRoom, renewRoomCode, transferOwnership, deleteRoom } from '@/lib/api/group';
import { roomKeys } from '@/lib/api/rooms';
import { authError } from '@/lib/api/errors';
import { useFeatures } from '@/lib/api/features';

export function RoomSettingsForm({ group }: { group: Group }) {
  const t = useTheme();
  const tr = useT();
  const features = useFeatures();
  const qc = useQueryClient();
  const { me } = useSession();
  const roomGone = useRoomGone();
  const [dialog, setDialog] = useState<'name' | 'delay' | 'owner' | null>(null);
  const close = () => setDialog(null);

  // Optimistic: the switch moves at once, and springs back if the server says no.
  const save = useMutation({
    meta: { quiet: true },
    mutationFn: (body: RoomPatch) => updateRoom(group.id, body),
    onMutate: async (body) => {
      await qc.cancelQueries({ queryKey: groupKeys.all });
      const before = qc.getQueryData<Group>(groupKeys.all);
      if (before) {
        const { name, language, ...settings } = body;
        qc.setQueryData<Group>(groupKeys.all, {
          ...before,
          name: name ?? before.name,
          language: language ?? before.language,
          settings: { ...before.settings, ...settings },
        });
      }
      return { before };
    },
    onError: (_e, _body, ctx) => {
      if (ctx?.before) qc.setQueryData(groupKeys.all, ctx.before);
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: groupKeys.all });
      qc.invalidateQueries({ queryKey: roomKeys.all });
    },
  });
  const renew = useMutation({
    meta: { quiet: true },
    mutationFn: () => renewRoomCode(group.id),
    onSettled: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
  });
  // Once done, I'm no longer the owner: the screen steps back by itself.
  const transfer = useMutation({
    meta: { quiet: true },
    mutationFn: (id: number) => transferOwnership(group.id, id),
    onSuccess: close,
    onSettled: () => qc.invalidateQueries({ queryKey: groupKeys.all }),
  });

  const s = group.settings;
  // An older server has no pacing: nothing it could save.
  const pacing = s.owner_next !== undefined || s.next_delay !== undefined;
  const delay = s.next_delay ?? 0;
  const others = group.members.filter((m) => m.id !== me?.id);

  const rotate = async () => {
    if (await askConfirm(tr('room.invite.newCodeAsk'), tr('room.invite.newCodeMessage'), tr('room.invite.change'))) renew.mutate();
  };
  const remove = async () => {
    if (!(await askConfirm(tr('room.settings.deleteAsk', { name: group.name }), tr('room.settings.deleteMessage'), tr('common.delete')))) return;
    try {
      await deleteRoom(group.id);
    } catch (e) {
      return showToast(authError(e));
    }
    await roomGone();
  };

  return (
    <>
      <ErrorText>{save.isError ? tr('room.settings.notSaved') : renew.isError || transfer.isError ? tr('room.settings.failed') : null}</ErrorText>

      <SettingsSection title={tr('room.settings.general')} Icon={Settings2}>
        <SettingsRow
          label={tr('room.settings.name')}
          value={group.name}
          a11y={tr('room.settings.nameA11y', { name: group.name })}
          onPress={() => setDialog('name')}
        />
        <VibeRow value={s.vibe ?? null} onChange={(v) => save.mutate({ vibe: v ?? '' })} />
      </SettingsSection>

      {pacing && (
        <SettingsSection title={tr('room.settings.pace')} Icon={Hourglass}>
          <SettingsRow
            label={tr('room.settings.ownerNext')}
            right={<Toggle label="" a11y={tr('room.settings.ownerNext')} value={!!s.owner_next} onChange={(v) => save.mutate({ owner_next: v })} />}
          />
          <SettingsRow
            label={tr('room.settings.delay')}
            value={delayLabel(delay)}
            a11y={tr('room.settings.delayA11y', { value: delayLabel(delay) })}
            onPress={() => setDialog('delay')}
          />
        </SettingsSection>
      )}

      <SettingsSection title={tr('room.settings.votes')} Icon={Vote}>
        <SettingsRow
          label={tr('room.settings.skipVote')}
          right={<Toggle label="" a11y={tr('room.settings.skipVote')} value={s.skip_vote} onChange={(v) => save.mutate({ skip_vote: v })} />}
        />
        <SettingsRow
          label={tr('room.settings.kickVote')}
          right={<Toggle label="" a11y={tr('room.settings.kickVote')} value={s.kick_vote} onChange={(v) => save.mutate({ kick_vote: v })} />}
        />
        {/* Never for a party (its reveal is on a shared screen); absent from an older server. */}
        {features.anonymousVotes && group.kind !== 'party' && s.anonymous_votes && (
          <AnonymousSetting value={s.anonymous_votes} onChange={(v) => save.mutate({ anonymous_votes: v })} />
        )}
        {/* Le Tripot (docs/TRIPOT.md): rooms only, absent from an older server. */}
        {features.tripot && group.kind !== 'party' && s.tripot !== undefined && (
          <SettingsRow
            label={tr('tripot.setting')}
            right={<Toggle label="" a11y={tr('tripot.setting')} value={s.tripot} onChange={(v) => save.mutate({ tripot: v })} />}
          />
        )}
      </SettingsSection>

      <SettingsSection title={tr('room.settings.questions')} Icon={MessageCircleQuestion}>
        <SettingsRow
          label={tr('room.settings.whoAdds')}
          right={
            <Segmented
              label={tr('room.settings.whoAddsA11y')}
              value={s.questions_by}
              onChange={(v) => save.mutate({ questions_by: v })}
              options={[
                { id: 'everyone', name: tr('room.settings.everyone') },
                { id: 'owner', name: tr('room.settings.you') },
              ]}
            />
          }
        />
        {features.languages && (
          <SettingsRow
            label={tr('room.settings.language')}
            right={
              <Segmented
                label={tr('room.language.label')}
                value={group.language ?? 'fr'}
                onChange={(v) => save.mutate({ language: v })}
                options={[
                  { id: 'fr', name: tr('profile.language.fr') },
                  { id: 'en', name: tr('profile.language.en') },
                ]}
              />
            }
          />
        )}
      </SettingsSection>

      <SettingsSection title={tr('room.settings.invites')} Icon={Ticket}>
        <SettingsRow
          label={tr('room.settings.newCode')}
          onPress={rotate}
          right={renew.isPending ? <ActivityIndicator size="small" color={t.mut} /> : <RefreshCw size={18} color={t.mut} />}
        />
      </SettingsSection>

      <SettingsSection title={tr('room.settings.danger')} Icon={TriangleAlert} danger>
        {others.length > 0 && <SettingsRow label={tr('room.settings.handOver')} danger onPress={() => setDialog('owner')} />}
        <SettingsRow label={tr('room.settings.delete')} danger onPress={remove} right={<Trash2 size={18} color={t.tagInk} />} />
      </SettingsSection>

      <NameDialog
        visible={dialog === 'name'}
        name={group.name}
        onClose={close}
        onSave={(name) => {
          close();
          save.mutate({ name });
        }}
      />
      <DelayDialog
        visible={dialog === 'delay'}
        value={delay}
        onClose={close}
        onPick={(v) => {
          close();
          if (v !== delay) save.mutate({ next_delay: v });
        }}
      />
      <HandOverDialog
        visible={dialog === 'owner'}
        members={others}
        busy={transfer.isPending}
        onClose={close}
        onConfirm={(id) => transfer.mutate(id)}
      />
    </>
  );
}
