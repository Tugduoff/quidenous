import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Send } from 'lucide-react-native';
import { type ComponentRef, useRef, useState } from 'react';
import { Platform, Text, TextInput, type TextInputProps, View } from 'react-native';
import { AttachButton } from '@/components/chat/attach-button';
import { Bubble } from '@/components/chat/bubble';
import { PendingBubble } from '@/components/chat/media';
import { MediaViewer } from '@/components/chat/media-viewer';
import { MentionList, type Suggestion } from '@/components/chat/mention-list';
import { ReactionBar, WhoReacted } from '@/components/chat/reactions';
import { ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { ProfileSheet } from '@/components/stats/profile-sheet';
import { Card } from '@/components/ui/card';
import { DialogTitle } from '@/components/ui/dialog';
import { PressableScale } from '@/components/ui/pressable-scale';
import { isSendKey, SEND_KEY } from '@/components/ui/send-on-enter';
import { chatKeys, deleteMessage, type Message, type NewMessage, postMessage, restoreMessage, useMessages, useReact } from '@/lib/api/chat';
import { useFeatures } from '@/lib/api/features';
import { useGroup, useMembers } from '@/lib/api/group';
import { useBlocks } from '@/lib/api/moderation';
import { useStats } from '@/lib/api/stats';
import { newClientKey, usePending } from '@/lib/chat-outbox';
import { useAutoGrowTextArea } from '@/lib/hooks/use-auto-grow';
import { useT } from '@/lib/i18n';
import { useArrivals } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { insertMention } from '@/lib/utils/chat';
import { activeMention, everyone, matches, outgoing } from '@/lib/utils/mentions';

const MAX = 500;

/** The conversation under a result, oldest first. Part of the page's own scroll (no box of its own: one scroll at a time). */
export function ChatMessages({ roundId }: { roundId: number }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { me } = useSession();
  const { byId } = useMembers();
  const { data } = useMessages(roundId);
  const { data: group } = useGroup();
  const { moderation, chatReactions } = useFeatures();
  const { ids: blocked } = useBlocks();
  const react = useReact(roundId);
  const pending = usePending(roundId);
  // The photo or GIF open full screen (gone if its message is deleted meanwhile).
  const [viewing, setViewing] = useState<number | null>(null);
  const viewed = viewing != null ? (data?.find((m) => m.id === viewing) ?? null) : null;
  // Long-press on a reaction: who reacted with it (kept fresh by the live updates).
  const [who, setWho] = useState<{ id: number; emoji: string } | null>(null);
  const whoReaction = (who && data?.find((m) => m.id === who.id)?.reactions?.find((r) => r.emoji === who.emoji && r.count > 0)) || null;
  // My own messages, or anyone's in a room I own (a server that can delete them).
  const owner = group?.my_role === 'owner';
  const remove = (id: number) => async () => {
    await deleteMessage(roundId, id);
    qc.setQueryData<Message[]>(chatKeys.messages(roundId), (old) => old?.filter((x) => x.id !== id));
  };
  const restore = (id: number) => async () => {
    await restoreMessage(roundId, id);
    qc.setQueryData<Message[]>(chatKeys.messages(roundId), (old) => old?.map((x) => (x.id === id ? { ...x, hidden: false } : x)));
  };
  // Long-press on someone else's message: report it, block its author.
  const [target, setTarget] = useState<ModTarget | null>(null);
  // A tapped @mention opens that person's profile, from the all-time stats (fetched on the first tap).
  const [profileId, setProfileId] = useState<number | null>(null);
  const stats = useStats('all', profileId != null);
  const profiles = new Map((stats.data?.profiles ?? []).map((p) => [p.member_id, p]));
  const settled = !!stats.data && !stats.isPlaceholderData;
  const missing = profileId != null && ((settled && !profiles.has(profileId)) || stats.isError);
  // No profile for them (an older server, someone who left): nothing opens.
  if (missing) setProfileId(null);
  const authorId = (m: Message) => m.user_id ?? byId.get(m.member_id)?.user_id;
  // People I blocked: gone at once, live messages included (the server leaves them out after a reload).
  const messages = data?.filter((m) => !blocked.has(authorId(m) ?? -1));
  // Messages already there when the result opens just show; new ones rise in.
  const isNew = useArrivals(messages?.map((m) => m.id));
  // No messages yet: nothing to show, the box below already invites one.
  if (!messages?.length && !pending.length) return null;
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>
        {tr('chat.count', { count: (messages?.length ?? 0) + pending.length })}
      </Text>
      <View style={{ gap: 10 }}>
        {messages?.map((m) => (
          <Bubble
            key={m.id}
            m={m}
            author={byId.get(m.member_id)}
            userId={authorId(m)}
            fresh={isNew(m.id)}
            onReport={setTarget}
            onProfile={setProfileId}
            onDelete={moderation && (owner || m.member_id === me?.id) ? remove(m.id) : undefined}
            onRestore={m.hidden && owner ? restore(m.id) : undefined}
            onReact={chatReactions && !m.hidden ? (emoji) => react(m, emoji) : undefined}
            onWho={(emoji) => setWho({ id: m.id, emoji })}
            onOpen={(x) => setViewing(x.id)}
          />
        ))}
        {pending.map((p) => (
          <PendingBubble key={p.key} roundId={roundId} p={p} />
        ))}
      </View>
      <ModerationDialog
        target={target}
        onClose={() => setTarget(null)}
        header={
          chatReactions
            ? (at) =>
                at.onRestore ? (
                  <DialogTitle title={tr('chat.hidden')} />
                ) : (
                  <ReactionBar
                    reactions={data?.find((m) => m.id === at.id)?.reactions}
                    onPick={(emoji) => {
                      const m = data?.find((x) => x.id === at.id);
                      if (m) react(m, emoji);
                      setTarget(null);
                    }}
                  />
                )
            : undefined
        }
      />
      <WhoReacted reaction={whoReaction} onClose={() => setWho(null)} />
      <MediaViewer message={viewed} onClose={() => setViewing(null)} />
      <ProfileSheet
        profile={profileId != null ? (profiles.get(profileId) ?? null) : null}
        byId={byId}
        onOpen={(id) => profiles.has(id) && setProfileId(id)}
        onClose={() => setProfileId(null)}
      />
    </View>
  );
}

/**
 * The message box, meant for the screen footer (always reachable, above Suivante, rises with the keyboard).
 * Typing `@` offers the people of the room (and `@tous`) right above the field.
 */
export function ChatInput({ roundId, onFocus, onSent }: { roundId: number; onFocus?: () => void; onSent?: () => void }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { me } = useSession();
  const { members } = useMembers();
  const features = useFeatures();
  const input = useRef<ComponentRef<typeof TextInput>>(null);
  const [text, setText] = useState('');
  // The caret as the field last reported it (null: nothing reported yet, read as the end of the text).
  const [sel, setSel] = useState<{ start: number; end: number } | null>(null);
  // People picked from the list since the last message.
  const [picked, setPicked] = useState<number[]>([]);
  // The `@` whose list was closed (Escape): stays closed until another one is typed.
  const [closed, setClosed] = useState<number | null>(null);
  const [highlight, setHighlight] = useState({ key: '', i: 0 });

  // The web's textarea keeps its height as the text grows or shrinks: fit it to the text (min/max height still apply).
  useAutoGrowTextArea(input, text);

  const others = members.filter((m) => m.id !== me?.id);
  const caret = sel ? Math.min(sel.end, text.length) : text.length;
  const typing = !sel || sel.start === sel.end ? activeMention(text, caret) : null;
  const rows: Suggestion[] =
    !typing || typing.at === closed || !others.length
      ? []
      : [
          ...(matches(everyone(), typing.query) ? [{ key: 'all', insert: `@${everyone()}` }] : []),
          ...others.filter((m) => matches(m.name, typing.query)).map((m) => ({ key: `m${m.id}`, insert: `@${m.name}`, member: m })),
        ];
  const open = rows.length > 0;
  // The highlighted row (Enter, Tab, the keyboard's send key pick it) goes back to the first person as the query changes:
  // pinging the whole room takes a deliberate choice.
  const listKey = typing ? `${typing.at}:${typing.query}` : '';
  const first = rows.length > 1 && !rows[0].member ? 1 : 0;
  const active = highlight.key === listKey ? Math.min(highlight.i, rows.length - 1) : first;
  const move = (step: number) => setHighlight({ key: listKey, i: (active + step + rows.length) % rows.length });

  const pick = (row: Suggestion) => {
    if (!typing) return;
    const { next, pos } = insertMention(text, typing, row.insert, MAX);
    setText(next);
    setSel({ start: pos, end: pos });
    const picked = row.member?.id;
    if (picked != null) setPicked((p) => (p.includes(picked) ? p : [...p, picked]));
    // Keep typing right after the name: the field keeps the focus (a click on the web took it), the caret goes after the space.
    requestAnimationFrame(() => {
      const el = input.current as (ComponentRef<typeof TextInput> & { setSelectionRange?: (a: number, b: number) => void }) | null;
      el?.focus();
      if (Platform.OS === 'web') el?.setSelectionRange?.(pos, pos);
      else el?.setSelection?.(pos, pos);
    });
  };

  // One id per message being sent: a retry of the same text (its answer lost) is posted once.
  const draft = useRef<{ text: string; key: string } | null>(null);
  const keyFor = (body: string) => {
    if (draft.current?.text !== body) draft.current = { text: body, key: newClientKey() };
    return draft.current.key;
  };
  const send = useMutation({
    mutationFn: (body: NewMessage) => postMessage(roundId, body),
    onSuccess: (m) => {
      draft.current = null;
      setText('');
      setSel(null);
      setPicked([]);
      setClosed(null);
      onSent?.();
      qc.setQueryData<Message[]>(chatKeys.messages(roundId), (old) => (old?.some((x) => x.id === m.id) ? old : [...(old ?? []), m]));
    },
  });
  const ok = text.trim().length > 0 && !send.isPending;
  const submit = () => {
    if (!ok) return;
    const body = text.trim();
    const { mentions, everyone } = outgoing(body, picked, others);
    // Only what's there (an older server ignores the key): an older server gets the same body as before.
    send.mutate({ text: body, ...(mentions.length ? { mentions } : {}), ...(everyone ? { everyone } : {}), client_key: keyFor(body) });
  };
  // While the list is open, the keys drive it: Enter / Tab / the keyboard's send key pick, arrows move, Escape closes.
  const sendOrPick = () => (open ? pick(rows[active]) : submit());
  const onKeyPress: TextInputProps['onKeyPress'] = (e) => {
    const key = e.nativeEvent as typeof e.nativeEvent & { shiftKey?: boolean; isComposing?: boolean };
    if (open && Platform.OS === 'web' && !key.isComposing) {
      const act: Record<string, (() => void) | undefined> = {
        ArrowDown: () => move(1),
        ArrowUp: () => move(-1),
        Tab: () => pick(rows[active]),
        Escape: () => setClosed(typing?.at ?? null),
      };
      const run = act[key.key];
      if (run) {
        e.preventDefault();
        run();
        return;
      }
    }
    if (isSendKey(e)) {
      e.preventDefault();
      sendOrPick();
    }
  };

  return (
    <Card style={{ padding: 6, gap: 4 }}>
      {open && <MentionList rows={rows} active={active} onPick={pick} onHover={(i) => setHighlight({ key: listKey, i })} />}
      <View style={{ flexDirection: 'row', alignItems: 'flex-end', gap: 8, paddingLeft: features.chatImages || features.chatGifs ? 0 : 10 }}>
        <AttachButton
          roundId={roundId}
          draft={text}
          picked={picked}
          others={others}
          onUseDraft={() => {
            setText('');
            setSel(null);
            setPicked([]);
            setClosed(null);
          }}
          onSent={onSent}
        />
        <TextInput
          ref={input}
          accessibilityLabel={tr('chat.inputLabel')}
          placeholder={tr('chat.placeholder')}
          placeholderTextColor={t.mut}
          value={text}
          onChangeText={(v) => {
            setText(v);
            // The closed list's `@` is gone: the next one opens again.
            if (closed != null && v[closed] !== '@') setClosed(null);
          }}
          onSelectionChange={(e) => setSel(e.nativeEvent.selection)}
          onFocus={onFocus}
          multiline
          // A textarea is two rows tall by default on the web: one row, grown with the text below.
          rows={1}
          maxLength={MAX}
          {...SEND_KEY}
          onSubmitEditing={sendOrPick}
          onKeyPress={onKeyPress}
          // Under 16px, Safari on iPhone zooms the page in when the field gets the focus.
          style={{ flex: 1, minWidth: 0, minHeight: 40, maxHeight: 100, paddingVertical: 10, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
        />
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr('common.send')}
          disabled={!ok}
          onPress={submit}
          hitSlop={4}
          style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: ok ? t.acc : t.card2 }}>
          <Send size={18} color={ok ? t.accTxt : t.mut} />
        </PressableScale>
      </View>
    </Card>
  );
}
