import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import {
  Check,
  ChevronRight,
  Hourglass,
  type LucideIcon,
  MessageCircleQuestion,
  RefreshCw,
  Settings2,
  Ticket,
  TriangleAlert,
  Trash2,
  Vote,
} from 'lucide-react-native';
import { type ReactNode, useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, Text, TextInput, View } from 'react-native';

import { BackButton, ErrorText } from '@/components/auth-ui';
import { Screen } from '@/components/screen';
import { askConfirm, Avatar, Button, Card, Dialog, DialogActions, DialogTitle, Display, PressableScale, Toggle } from '@/components/ui';
import { VibeCards, vibeIcon } from '@/components/vibes';
import { api, type Group, type Member } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { keys, useGroup, useVibes } from '@/lib/queries';
import { useRoomGone } from '@/lib/rooms';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

type Patch = Partial<Group['settings']> & { name?: string };

/** The waits the server accepts between two questions, in seconds. */
const DELAYS = [0, 900, 3600, 10800, 43200, 86400];
const delayLabel = (s: number) => (!s ? 'Aucun' : s < 3600 ? `${Math.round(s / 60)} min` : `${Math.round(s / 3600)} h`);

/** A small titled block: grey header with its icon (red for the danger zone), the rows in a card. */
function Section({ title, Icon, danger, children }: { title: string; Icon: LucideIcon; danger?: boolean; children: ReactNode }) {
  const t = useTheme();
  const color = danger ? t.tag : t.mut;
  return (
    <View style={{ gap: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, marginLeft: 4 }}>
        <Icon size={14} color={color} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color }}>{title}</Text>
      </View>
      <Card style={{ padding: 16, gap: 14 }}>{children}</Card>
    </View>
  );
}

/** One setting on one line: its label, then a control (or, with `onPress`, a value and a chevron). */
function Row({
  label,
  value,
  right,
  onPress,
  a11y,
  danger,
}: {
  label: string;
  value?: string;
  right?: ReactNode;
  onPress?: () => void;
  a11y?: string;
  danger?: boolean;
}) {
  const t = useTheme();
  const body = (
    <>
      <Text numberOfLines={1} style={{ flexShrink: 0, maxWidth: '70%', fontFamily: t.font.semibold, fontSize: 15, color: danger ? t.tag : t.txt }}>
        {label}
      </Text>
      <View style={{ flex: 1, minWidth: 0, flexDirection: 'row', alignItems: 'center', justifyContent: 'flex-end', gap: 8 }}>
        {value != null && (
          <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>
            {value}
          </Text>
        )}
        {right ?? (onPress && <ChevronRight size={18} color={danger ? t.tag : t.mut} />)}
      </View>
    </>
  );
  const style = { flexDirection: 'row', alignItems: 'center', gap: 12, minHeight: 36 } as const;
  if (!onPress) return <View style={style}>{body}</View>;
  return (
    <PressableScale scale={0.98} accessibilityRole="button" accessibilityLabel={a11y ?? label} onPress={onPress} style={style}>
      {body}
    </PressableScale>
  );
}

/** A compact two-or-more way choice (pills in a track). */
function Choice<T extends string>({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: { id: T; name: string }[];
  value: T;
  onChange: (v: T) => void;
}) {
  const t = useTheme();
  return (
    <View
      accessibilityRole="radiogroup"
      accessibilityLabel={label}
      style={{ flexDirection: 'row', padding: 3, gap: 2, borderRadius: t.radius.pill, backgroundColor: t.card2 }}>
      {options.map((o) => {
        const on = o.id === value;
        return (
          <Pressable
            key={o.id}
            accessibilityRole="radio"
            accessibilityState={{ selected: on }}
            accessibilityLabel={o.name}
            onPress={() => !on && onChange(o.id)}
            style={{
              height: 30,
              paddingHorizontal: 12,
              borderRadius: t.radius.pill,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: on ? t.acc : 'transparent',
            }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: on ? t.accTxt : t.mut }}>{o.name}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}

/** Rename in a dialog. */
function NameDialog({ visible, name, onClose, onSave }: { visible: boolean; name: string; onClose: () => void; onSave: (name: string) => void }) {
  const t = useTheme();
  const [draft, setDraft] = useState(name);
  // Each opening starts from the current name.
  useEffect(() => {
    if (visible) setDraft(name);
  }, [visible, name]);
  const ok = !!draft.trim() && draft.trim() !== name;
  const submit = () => ok && onSave(draft.trim());
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title="Nom du salon" />
      <TextInput
        accessibilityLabel="Nom du salon"
        value={draft}
        onChangeText={setDraft}
        maxLength={40}
        autoFocus
        returnKeyType="done"
        onSubmitEditing={submit}
        style={{
          height: 48,
          paddingHorizontal: 14,
          borderRadius: 14,
          backgroundColor: t.card2,
          fontFamily: t.font.semibold,
          fontSize: 16,
          color: t.txt,
        }}
      />
      <Button disabled={!ok} onPress={submit} style={{ height: 48 }}>
        Renommer
      </Button>
    </Dialog>
  );
}

/** The tone of the room's built-in questions (a server that has vibes). */
function VibeRow({ value, onChange }: { value: string | null; onChange: (vibe: string | null) => void }) {
  const t = useTheme();
  const features = useFeatures();
  const { data: vibes } = useVibes(features.vibes);
  const [open, setOpen] = useState(false);
  if (!features.vibes || !vibes?.length) return null;
  const current = vibes.find((v) => v.id === value);
  const Icon = vibeIcon(current?.icon);
  const pick = (v: string | null) => {
    setOpen(false);
    if (v !== value) onChange(v);
  };
  return (
    <>
      <Row
        label="Ambiance"
        a11y={`Ambiance : ${current?.name ?? 'aucune'}. Changer`}
        onPress={() => setOpen(true)}
        right={
          <>
            {current && <Icon size={16} color={t.accInk} />}
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: current ? t.txt : t.mut }}>
              {current?.name ?? 'Aucune'}
            </Text>
            <ChevronRight size={18} color={t.mut} />
          </>
        }
      />
      <Dialog visible={open} onClose={() => setOpen(false)}>
        <DialogTitle title="Ambiance" />
        <VibeCards vibes={vibes} value={value} onChange={pick} />
        <Pressable
          accessibilityRole="radio"
          accessibilityState={{ selected: !current }}
          onPress={() => pick(null)}
          style={{ height: 44, flexDirection: 'row', gap: 6, alignItems: 'center', justifyContent: 'center' }}>
          {!current && <Check size={16} color={t.accInk} />}
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: current ? t.mut : t.accInk }}>Aucune</Text>
        </Pressable>
      </Dialog>
    </>
  );
}

/** Pick the wait before the next question. */
function DelayDialog({ visible, value, onClose, onPick }: { visible: boolean; value: number; onClose: () => void; onPick: (s: number) => void }) {
  const t = useTheme();
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title="Délai avant la suivante" />
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8 }}>
        {DELAYS.map((s) => {
          const on = s === value;
          return (
            <Pressable
              key={s}
              accessibilityRole="radio"
              accessibilityState={{ selected: on }}
              onPress={() => onPick(s)}
              style={{
                width: '30%',
                flexGrow: 1,
                height: 44,
                borderRadius: 12,
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: on ? t.acc : t.card2,
              }}>
              <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: on ? t.accTxt : t.txt }}>{delayLabel(s)}</Text>
            </Pressable>
          );
        })}
      </View>
    </Dialog>
  );
}

/** Pick the new owner, then confirm, in the same dialog. */
function HandOverDialog({
  visible,
  members,
  busy,
  onClose,
  onConfirm,
}: {
  visible: boolean;
  members: Member[];
  busy: boolean;
  onClose: () => void;
  onConfirm: (id: number) => void;
}) {
  const t = useTheme();
  const [heir, setHeir] = useState<Member | null>(null);
  useEffect(() => {
    if (visible) setHeir(null);
  }, [visible]);
  return (
    <Dialog visible={visible} onClose={onClose}>
      {heir ? (
        <>
          <DialogTitle title="Passer la main ?" message={`${heir.name} devient propriétaire du salon, et toi simple membre.`} />
          <DialogActions confirmLabel="Confirmer" busy={busy} onConfirm={() => onConfirm(heir.id)} onCancel={() => setHeir(null)} />
        </>
      ) : (
        <>
          <DialogTitle title="Passer la main" />
          <View style={{ gap: 6 }}>
            {members.map((m) => (
              <PressableScale
                key={m.id}
                scale={0.98}
                accessibilityRole="button"
                accessibilityLabel={`Rendre ${m.name} propriétaire`}
                onPress={() => setHeir(m)}
                style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 10, borderRadius: 16, backgroundColor: t.card2 }}>
                <Avatar member={m} size={32} />
                <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                  {m.name}
                </Text>
                <ChevronRight size={18} color={t.mut} />
              </PressableScale>
            ))}
          </View>
        </>
      )}
    </Dialog>
  );
}

function Settings({ group }: { group: Group }) {
  const t = useTheme();
  const qc = useQueryClient();
  const { me } = useSession();
  const roomGone = useRoomGone();
  const [dialog, setDialog] = useState<'name' | 'delay' | 'owner' | null>(null);
  const close = () => setDialog(null);

  // Optimistic: the switch moves at once, and springs back if the server says no.
  const save = useMutation({
    mutationFn: (body: Patch) => api(`/rooms/${group.id}`, { method: 'PATCH', json: body }),
    onMutate: async (body) => {
      await qc.cancelQueries({ queryKey: keys.group });
      const before = qc.getQueryData<Group>(keys.group);
      if (before) {
        const { name, ...settings } = body;
        qc.setQueryData<Group>(keys.group, { ...before, name: name ?? before.name, settings: { ...before.settings, ...settings } });
      }
      return { before };
    },
    onError: (_e, _body, ctx) => {
      if (ctx?.before) qc.setQueryData(keys.group, ctx.before);
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: keys.group });
      qc.invalidateQueries({ queryKey: keys.rooms });
    },
  });
  const renew = useMutation({
    mutationFn: () => api(`/rooms/${group.id}/code`, { method: 'POST' }),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.group }),
  });
  // Once done, I'm no longer the owner: the screen steps back by itself.
  const transfer = useMutation({
    mutationFn: (id: number) => api(`/rooms/${group.id}/owner`, { method: 'POST', json: { member_id: id } }),
    onSuccess: close,
    onSettled: () => qc.invalidateQueries({ queryKey: keys.group }),
  });

  const s = group.settings;
  // An older server has no pacing: nothing it could save.
  const pacing = s.owner_next !== undefined || s.next_delay !== undefined;
  const delay = s.next_delay ?? 0;
  const others = group.members.filter((m) => m.id !== me?.id);

  const rotate = async () => {
    if (await askConfirm('Nouveau code ?', "L'ancien code et les liens déjà envoyés ne marcheront plus.", 'Changer')) renew.mutate();
  };
  const remove = async () => {
    if (!(await askConfirm(`Supprimer « ${group.name} » ?`, 'Le salon disparaît pour tout le monde.', 'Supprimer'))) return;
    await api(`/rooms/${group.id}`, { method: 'DELETE' });
    await roomGone();
  };

  return (
    <>
      <ErrorText>{save.isError ? 'Réglage non enregistré' : renew.isError || transfer.isError ? 'Action impossible' : null}</ErrorText>

      <Section title="Général" Icon={Settings2}>
        <Row label="Nom" value={group.name} a11y={`Nom du salon : ${group.name}. Changer`} onPress={() => setDialog('name')} />
        <VibeRow value={s.vibe ?? null} onChange={(v) => save.mutate({ vibe: v ?? '' })} />
      </Section>

      {pacing && (
        <Section title="Rythme" Icon={Hourglass}>
          <Row
            label="Toi seul lances la suivante"
            right={<Toggle label="" a11y="Toi seul lances la suivante" value={!!s.owner_next} onChange={(v) => save.mutate({ owner_next: v })} />}
          />
          <Row
            label="Délai avant la suivante"
            value={delayLabel(delay)}
            a11y={`Délai avant la suivante : ${delayLabel(delay)}. Changer`}
            onPress={() => setDialog('delay')}
          />
        </Section>
      )}

      <Section title="Votes" Icon={Vote}>
        <Row
          label="Passer à la majorité"
          right={<Toggle label="" a11y="Passer à la majorité" value={s.skip_vote} onChange={(v) => save.mutate({ skip_vote: v })} />}
        />
        <Row
          label="Exclure à la majorité"
          right={<Toggle label="" a11y="Exclure à la majorité" value={s.kick_vote} onChange={(v) => save.mutate({ kick_vote: v })} />}
        />
      </Section>

      <Section title="Questions" Icon={MessageCircleQuestion}>
        <Row
          label="Qui peut en ajouter"
          right={
            <Choice
              label="Qui peut ajouter des questions"
              value={s.questions_by}
              onChange={(v) => save.mutate({ questions_by: v })}
              options={[
                { id: 'everyone', name: 'Tous' },
                { id: 'owner', name: 'Toi' },
              ]}
            />
          }
        />
      </Section>

      <Section title="Invitations" Icon={Ticket}>
        <Row
          label="Nouveau code d'invitation"
          onPress={rotate}
          right={renew.isPending ? <ActivityIndicator size="small" color={t.mut} /> : <RefreshCw size={18} color={t.mut} />}
        />
      </Section>

      <Section title="Zone sensible" Icon={TriangleAlert} danger>
        {others.length > 0 && <Row label="Passer la main" danger onPress={() => setDialog('owner')} />}
        <Row label="Supprimer le salon" danger onPress={remove} right={<Trash2 size={18} color={t.tag} />} />
      </Section>

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

/** Group > gear (owner only): the room's name, tone, pacing, votes, who adds questions, the invite code, and the danger zone. */
export default function RoomSettings() {
  const t = useTheme();
  const { data: group } = useGroup();
  const owner = group?.my_role === 'owner';
  // Not (or no longer) the owner: nothing to see here.
  useEffect(() => {
    if (group && !owner) {
      if (router.canGoBack()) router.back();
      else router.replace('/group');
    }
  }, [group, owner]);
  return (
    <Screen tabs={false}>
      <BackButton />
      <View style={{ gap: 4 }}>
        <Display size={30}>Paramètres du salon</Display>
        {owner && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
            {group.name}
          </Text>
        )}
      </View>
      {!group ? <ActivityIndicator color={t.mut} style={{ marginTop: 24 }} /> : owner ? <Settings group={group} /> : null}
    </Screen>
  );
}
