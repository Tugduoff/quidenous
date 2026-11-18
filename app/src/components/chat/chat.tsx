import { useMutation, useQueryClient } from '@tanstack/react-query';
import * as Haptics from 'expo-haptics';
import { EyeOff, Send, Users } from 'lucide-react-native';
import { type ComponentRef, useEffect, useRef, useState } from 'react';
import { Platform, Pressable, ScrollView, Text, TextInput, type TextInputProps, type TextStyle, View, type ViewStyle } from 'react-native';
import Animated from 'react-native-reanimated';
import { AttachButton, MediaViewer, MessageMediaFrame, PendingBubble } from '@/components/chat/media';
import { HeartBurst, ReactionBar, ReactionChips, WhoReacted } from '@/components/chat/reactions';
import { ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { ProfileSheet } from '@/components/stats/profile-sheet';
import { api, type Member, type Message } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { mediaOf, usePending } from '@/lib/chat-media';
import { dateFormat, useT } from '@/lib/i18n';
import { activeMention, everyone, matches, outgoing, segments } from '@/lib/mentions';
import { useBlocks } from '@/lib/moderation';
import { motion, useArrivals } from '@/lib/motion';
import { keys, useGroup, useMembers, useMessages, useStats } from '@/lib/queries';
import { HEART, useReact } from '@/lib/reactions';
import { sendOnEnter } from '@/lib/submit';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import { DialogTitle } from '@/components/ui/dialog';

const MAX = 500;
/** One suggestion row; the list shows about five and a half, so a longer one visibly scrolls. */
const ROW = 40;

/** `#rrggbb` → the same colour, faint (a mention's background). */
const soft = (color: string) => (/^#[0-9a-f]{6}$/i.test(color) ? `${color}33` : 'rgba(127,127,127,0.18)');
/** Two taps closer than this on a message: ❤️. */
const DOUBLE_TAP_MS = 280;

/** Web: a double tap (❤️) doesn't select a word, a long press doesn't open the iOS text callout over the menu. */
const NO_SELECT = Platform.OS === 'web' ? ({ userSelect: 'none', WebkitTouchCallout: 'none' } as unknown as ViewStyle) : null;

const haptic = () => {
  if (Platform.OS !== 'web') Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
};

/** One message. A component of its own, so a new message does not redraw the others. */
function Bubble({
  m,
  author,
  userId,
  fresh,
  onReport,
  onProfile,
  onDelete,
  onRestore,
  onReact,
  onWho,
  onOpen,
}: {
  m: Message;
  author?: Member;
  userId?: number | null;
  fresh: boolean;
  onReport: (target: ModTarget) => void;
  onProfile: (id: number) => void;
  /** I may delete it (mine, or I own the room): offered on long-press. */
  onDelete?: () => Promise<unknown>;
  /** Hidden by reports, in a room I own: I may bring it back (long-press). */
  onRestore?: () => Promise<unknown>;
  /** The server has reactions: toggles mine (true: added). The long-press menu then opens on every message, mine too. */
  onReact?: (emoji: string) => boolean;
  onWho: (emoji: string) => void;
  /** A photo or GIF was tapped: full screen. */
  onOpen: (m: Message) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const { me } = useSession();
  const { byId } = useMembers();
  const mine = m.member_id === me?.id;
  const mentionStyle = (mine: boolean, color: string): TextStyle =>
    // On my own bubble (my colour behind): dark text on a darker wash, like the rest of the bubble.
    mine
      ? { fontFamily: t.font.bold, color: '#121216', backgroundColor: 'rgba(0,0,0,0.12)', borderRadius: 5, paddingHorizontal: 3 }
      : { fontFamily: t.font.bold, color, backgroundColor: soft(color), borderRadius: 5, paddingHorizontal: 3 };
  const target = (): ModTarget =>
    mine
      ? { kind: 'message', id: m.id, own: true, onDelete, onRestore }
      : {
          kind: 'message',
          id: m.id,
          person:
            author && userId != null
              ? { user_id: userId, name: author.name, color: author.color, photo: author.photo_url, avatar: author.avatar }
              : undefined,
          onDelete,
          onRestore,
        };
  const bubble = useRef<ComponentRef<typeof View>>(null);
  // Double tap: the time of the last tap, and of the last tap on a mention (that one opens a profile, nothing else).
  const lastTap = useRef(0);
  const mentionTap = useRef(0);
  const [burst, setBurst] = useState(0);
  // A photo or a GIF (null: a text, or a kind from a newer server, shown as its text).
  const media = mediaOf(m);
  // A tap on a photo opens it, once it's clear it isn't the first of a double tap (❤️).
  const opening = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(opening.current), []);
  const heart = () => {
    if (onReact?.(HEART)) setBurst((b) => b + 1);
  };
  const onPress =
    onReact || media
      ? () => {
          const now = Date.now();
          if (now - mentionTap.current < 300) return;
          if (onReact && now - lastTap.current < DOUBLE_TAP_MS) {
            lastTap.current = 0;
            clearTimeout(opening.current);
            heart();
            return;
          }
          lastTap.current = now;
          if (!media) return;
          if (onReact) opening.current = setTimeout(() => onOpen(m), DOUBLE_TAP_MS);
          else onOpen(m);
        }
      : undefined;
  // With reactions: a menu floating by the message (reactions on top, then delete / report / block).
  const menu = () => {
    lastTap.current = 0;
    haptic();
    const at = target();
    if (!bubble.current) return onReport(at);
    bubble.current.measureInWindow((_x, y, _w, height) =>
      onReport(Number.isFinite(y) && height > 0 ? { ...at, anchor: { y, height, side: mine ? 'right' : 'left' } } : at),
    );
  };
  const onLongPress = onReact || onRestore ? menu : mine ? onDelete && (() => onReport(target())) : () => onReport(target());
  const hint = onReact
    ? tr(mine ? (onDelete ? 'chat.reactions.ownHint' : 'chat.reactions.ownOnlyHint') : 'chat.reactions.hint')
    : mine
      ? onDelete
        ? tr('chat.longPressOwnHint')
        : undefined
      : tr('chat.longPressHint');
  const reactions = m.reactions?.filter((r) => r.count > 0) ?? [];
  const ink = mine ? '#121216' : t.txt;
  // The text, or a photo's caption, with its mentions.
  const rich = (text: string) => (
    <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 20, color: ink, paddingHorizontal: media ? 10 : 0 }}>
      {segments(
        text,
        (m.mentions ?? []).map((id) => byId.get(id)).filter((x): x is Member => !!x),
        !!m.everyone,
      ).map((p, i) =>
        p.member ? (
          <Text
            key={i}
            accessibilityRole="link"
            accessibilityHint={tr('chat.seeProfile')}
            onPress={() => {
              mentionTap.current = Date.now();
              onProfile(p.member!.id);
            }}
            onLongPress={onLongPress}
            style={mentionStyle(mine, p.member.color)}>
            {p.text}
          </Text>
        ) : p.everyone ? (
          <Text key={i} style={mine ? mentionStyle(true, '') : { ...mentionStyle(false, t.accInk), backgroundColor: t.accSoft }}>
            {p.text}
          </Text>
        ) : (
          p.text
        ),
      )}
    </Text>
  );
  const mediaLabel = media ? tr(m.kind === 'gif' ? 'chat.media.gifBy' : 'chat.media.photoBy', { name: (mine ? me?.name : author?.name) ?? '?' }) : '';
  return (
    <Animated.View key={m.id} entering={fresh ? motion.rise : undefined} style={{ gap: 4 }}>
      <View style={{ flexDirection: mine ? 'row-reverse' : 'row', gap: 8, alignItems: 'flex-end', opacity: m.hidden ? 0.45 : 1 }}>
        {!mine && <Avatar member={author} size={28} />}
        <Pressable
          ref={bubble}
          onPress={onPress}
          onLongPress={onLongPress}
          delayLongPress={400}
          accessibilityLabel={media ? (m.caption ? `${mediaLabel}, ${m.caption}` : mediaLabel) : undefined}
          accessibilityHint={media ? [tr('chat.media.open'), hint].filter(Boolean).join('. ') : hint}
          accessibilityActions={onReact ? [{ name: 'heart', label: tr('chat.reactions.react', { emoji: HEART }) }] : undefined}
          onAccessibilityAction={onReact ? (e) => e.nativeEvent.actionName === 'heart' && heart() : undefined}
          style={[
            NO_SELECT,
            {
              maxWidth: '78%',
              // A photo: a thin frame of the bubble around it, the name and caption inset like a text.
              paddingHorizontal: media ? 4 : 14,
              paddingTop: media ? 4 : 9,
              paddingBottom: media ? (m.caption ? 7 : 4) : 9,
              borderRadius: 18,
              borderBottomLeftRadius: mine ? 18 : 6,
              borderBottomRightRadius: mine ? 6 : 18,
              // My own bubbles in my colour: lime is for actions (Suivante, Envoyer).
              backgroundColor: mine ? (me?.color ?? t.card2) : t.card,
              gap: 2,
            },
          ]}>
          {!mine && (
            <Text
              numberOfLines={1}
              style={{
                fontFamily: t.font.bold,
                fontSize: 12,
                color: author?.color ?? t.mut,
                paddingHorizontal: media ? 10 : 0,
                paddingTop: media ? 3 : 0,
                paddingBottom: media ? 2 : 0,
              }}>
              {author?.name ?? '?'}
            </Text>
          )}
          {media ? (
            <>
              <MessageMediaFrame m={m} media={media} label={mediaLabel} />
              {!!m.caption && rich(m.caption)}
            </>
          ) : (
            rich(m.text)
          )}
          {/* A photo without a caption has its time on it. */}
          {(!media || !!m.caption) && (
            <Text
              style={{
                fontFamily: t.font.regular,
                fontSize: 10,
                color: mine ? '#121216' : t.mut,
                opacity: 0.7,
                alignSelf: 'flex-end',
                paddingHorizontal: media ? 10 : 0,
              }}>
              {dateFormat({ hour: '2-digit', minute: '2-digit' }).format(new Date(m.created_at))}
            </Text>
          )}
          {!!onReact && <HeartBurst trigger={burst} />}
        </Pressable>
      </View>
      {reactions.length > 0 && <ReactionChips reactions={reactions} mine={mine} onToggle={onReact && ((e) => void onReact(e))} onWho={onWho} />}
      {/* Hidden by reports: only I (the owner) see it, faded, until I restore or delete it. */}
      {!!m.hidden && (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 5, alignSelf: mine ? 'flex-end' : 'flex-start', marginLeft: mine ? 0 : 36 }}>
          <EyeOff size={13} color={t.mut} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.mut }}>{tr('chat.hidden')}</Text>
        </View>
      )}
    </Animated.View>
  );
}

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
    await api(`/rounds/${roundId}/messages/${id}`, { method: 'DELETE' });
    qc.setQueryData<Message[]>(keys.messages(roundId), (old) => old?.filter((x) => x.id !== id));
  };
  const restore = (id: number) => async () => {
    await api(`/rounds/${roundId}/messages/${id}/restore`, { method: 'POST' });
    qc.setQueryData<Message[]>(keys.messages(roundId), (old) => old?.map((x) => (x.id === id ? { ...x, hidden: false } : x)));
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
  useEffect(() => {
    if (missing) setProfileId(null);
  }, [missing]);
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

type Suggestion = { key: string; insert: string; member?: Member };

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
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    const el = input.current as unknown as HTMLTextAreaElement | null;
    if (!el?.style) return;
    el.style.height = '0px';
    el.style.height = `${el.scrollHeight}px`;
  }, [text]);

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
    // "@lo|u" → "@Louis |": the whole word typed so far is replaced, one space after (not two).
    const after = text.slice(typing.at + 1 + typing.query.length).replace(/^\S*/, '');
    const head = `${text.slice(0, typing.at)}${row.insert} `;
    const next = (head + after.replace(/^ /, '')).slice(0, MAX);
    const pos = Math.min(head.length, next.length);
    setText(next);
    setSel({ start: pos, end: pos });
    if (row.member) setPicked((p) => (p.includes(row.member!.id) ? p : [...p, row.member!.id]));
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
    if (draft.current?.text !== body) draft.current = { text: body, key: `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}` };
    return draft.current.key;
  };
  const send = useMutation({
    mutationFn: (body: { text: string; mentions?: number[]; everyone?: boolean; client_key: string }) =>
      api<Message>(`/rounds/${roundId}/messages`, { method: 'POST', json: body }),
    onSuccess: (m) => {
      draft.current = null;
      setText('');
      setSel(null);
      setPicked([]);
      setClosed(null);
      onSent?.();
      qc.setQueryData<Message[]>(keys.messages(roundId), (old) => (old?.some((x) => x.id === m.id) ? old : [...(old ?? []), m]));
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
  const enter = sendOnEnter(() => (open ? pick(rows[active]) : submit()));
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
    enter.onKeyPress?.(e);
  };

  return (
    <Card style={{ padding: 6, gap: 4 }}>
      {open && (
        <Animated.View entering={motion.rise} style={{ borderBottomWidth: 1, borderColor: t.card2, paddingBottom: 4 }}>
          <ScrollView style={{ maxHeight: ROW * 5.5 }} keyboardShouldPersistTaps="handled" nestedScrollEnabled>
            {rows.map((r, i) => (
              <Pressable
                key={r.key}
                accessibilityRole="button"
                accessibilityLabel={r.member ? tr('chat.mention', { name: r.member.name }) : tr('chat.mentionAll')}
                onPress={() => pick(r)}
                onHoverIn={() => setHighlight({ key: listKey, i })}
                style={{
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 10,
                  height: ROW,
                  paddingHorizontal: 8,
                  borderRadius: 14,
                  backgroundColor: i === active ? t.card2 : undefined,
                }}>
                {r.member ? (
                  <Avatar member={r.member} size={28} />
                ) : (
                  <View
                    style={{ width: 28, height: 28, borderRadius: 14, backgroundColor: t.accSoft, alignItems: 'center', justifyContent: 'center' }}>
                    <Users size={15} color={t.accInk} />
                  </View>
                )}
                <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                  {r.member ? r.member.name : everyone()}
                </Text>
                {!r.member && <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('chat.everyoneRow')}</Text>}
              </Pressable>
            ))}
          </ScrollView>
        </Animated.View>
      )}
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
          {...enter}
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
