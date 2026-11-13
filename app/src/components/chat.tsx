import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Send, Users } from 'lucide-react-native';
import { type ComponentRef, useEffect, useRef, useState } from 'react';
import { Platform, Pressable, ScrollView, Text, TextInput, type TextInputProps, type TextStyle, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { ModerationDialog, type ModTarget } from '@/components/moderation';
import { ProfileSheet } from '@/components/stats/profile-sheet';
import { Avatar, Card, PressableScale } from '@/components/ui';
import { api, type Member, type Message } from '@/lib/api';
import { activeMention, EVERYONE, matches, outgoing, segments } from '@/lib/mentions';
import { useBlocks } from '@/lib/moderation';
import { motion, useArrivals } from '@/lib/motion';
import { keys, useMembers, useMessages, useStats } from '@/lib/queries';
import { sendOnEnter } from '@/lib/submit';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

const TIME = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' });
const MAX = 500;
/** One suggestion row; the list shows about five and a half, so a longer one visibly scrolls. */
const ROW = 40;

/** `#rrggbb` → the same colour, faint (a mention's background). */
const soft = (color: string) => (/^#[0-9a-f]{6}$/i.test(color) ? `${color}33` : 'rgba(127,127,127,0.18)');

/** The conversation under a result, oldest first. Part of the page's own scroll (no box of its own: one scroll at a time). */
export function ChatMessages({ roundId }: { roundId: number }) {
  const t = useTheme();
  const { me } = useSession();
  const { byId } = useMembers();
  const { data } = useMessages(roundId);
  const { ids: blocked } = useBlocks();
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
  const mentionStyle = (mine: boolean, color: string): TextStyle =>
    // On my own bubble (my colour behind): dark text on a darker wash, like the rest of the bubble.
    mine
      ? { fontFamily: t.font.bold, color: '#121216', backgroundColor: 'rgba(0,0,0,0.12)', borderRadius: 5, paddingHorizontal: 3 }
      : { fontFamily: t.font.bold, color, backgroundColor: soft(color), borderRadius: 5, paddingHorizontal: 3 };
  const authorId = (m: Message) => m.user_id ?? byId.get(m.member_id)?.user_id;
  // People I blocked: gone at once, live messages included (the server leaves them out after a reload).
  const messages = data?.filter((m) => !blocked.has(authorId(m) ?? -1));
  // Messages already there when the result opens just show; new ones rise in.
  const isNew = useArrivals(messages?.map((m) => m.id));
  // No messages yet: nothing to show, the box below already invites one.
  if (!messages?.length) return null;
  return (
    <View style={{ gap: 10 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>
        {`${messages.length} message${messages.length > 1 ? 's' : ''}`}
      </Text>
      <View style={{ gap: 10 }}>
        {messages.map((m) => {
          const author = byId.get(m.member_id);
          const mine = m.member_id === me?.id;
          const userId = authorId(m);
          const onLongPress = mine
            ? undefined
            : () =>
                setTarget({
                  kind: 'message',
                  id: m.id,
                  person: author && userId != null ? { user_id: userId, name: author.name, color: author.color, photo: author.photo_url, avatar: author.avatar } : undefined,
                });
          return (
            <Animated.View
              key={m.id}
              entering={isNew(m.id) ? motion.rise : undefined}
              style={{ flexDirection: mine ? 'row-reverse' : 'row', gap: 8, alignItems: 'flex-end' }}>
              {!mine && <Avatar member={author} size={28} />}
              <Pressable
                onLongPress={onLongPress}
                delayLongPress={400}
                accessibilityHint={mine ? undefined : 'Appui long : signaler ou bloquer'}
                style={{
                  maxWidth: '78%',
                  paddingHorizontal: 14,
                  paddingVertical: 9,
                  borderRadius: 18,
                  borderBottomLeftRadius: mine ? 18 : 6,
                  borderBottomRightRadius: mine ? 6 : 18,
                  // My own bubbles in my colour: lime is for actions (Suivante, Envoyer).
                  backgroundColor: mine ? (me?.color ?? t.card2) : t.card,
                  gap: 2,
                }}>
                {!mine && (
                  <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 12, color: author?.color ?? t.mut }}>
                    {author?.name ?? '?'}
                  </Text>
                )}
                <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 20, color: mine ? '#121216' : t.txt }}>
                  {segments(
                    m.text,
                    (m.mentions ?? []).map((id) => byId.get(id)).filter((x): x is Member => !!x),
                    !!m.everyone,
                  ).map((p, i) =>
                    p.member ? (
                      <Text
                        key={i}
                        accessibilityRole="link"
                        accessibilityHint="Voir son profil"
                        onPress={() => setProfileId(p.member!.id)}
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
                <Text style={{ fontFamily: t.font.regular, fontSize: 10, color: mine ? '#121216' : t.mut, opacity: 0.7, alignSelf: 'flex-end' }}>
                  {TIME.format(new Date(m.created_at))}
                </Text>
              </Pressable>
            </Animated.View>
          );
        })}
      </View>
      <ModerationDialog target={target} onClose={() => setTarget(null)} />
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
  const qc = useQueryClient();
  const { me } = useSession();
  const { members } = useMembers();
  const input = useRef<ComponentRef<typeof TextInput>>(null);
  const [text, setText] = useState('');
  // The caret as the field last reported it (null: nothing reported yet, read as the end of the text).
  const [sel, setSel] = useState<{ start: number; end: number } | null>(null);
  // People picked from the list since the last message.
  const [picked, setPicked] = useState<number[]>([]);
  // The `@` whose list was closed (Escape): stays closed until another one is typed.
  const [closed, setClosed] = useState<number | null>(null);
  const [highlight, setHighlight] = useState({ key: '', i: 0 });

  const others = members.filter((m) => m.id !== me?.id);
  const caret = sel ? Math.min(sel.end, text.length) : text.length;
  const typing = !sel || sel.start === sel.end ? activeMention(text, caret) : null;
  const rows: Suggestion[] =
    !typing || typing.at === closed || !others.length
      ? []
      : [
          ...(matches(EVERYONE, typing.query) ? [{ key: 'all', insert: `@${EVERYONE}` }] : []),
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

  const send = useMutation({
    mutationFn: (body: { text: string; mentions?: number[]; everyone?: boolean }) =>
      api<Message>(`/rounds/${roundId}/messages`, { method: 'POST', json: body }),
    onSuccess: (m) => {
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
    // Only what's there: an older server gets the same body as before.
    send.mutate({ text: body, ...(mentions.length ? { mentions } : {}), ...(everyone ? { everyone } : {}) });
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
                accessibilityLabel={r.member ? `Mentionner ${r.member.name}` : 'Mentionner tout le salon'}
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
                  {r.member ? r.member.name : EVERYONE}
                </Text>
                {!r.member && <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>Tout le salon</Text>}
              </Pressable>
            ))}
          </ScrollView>
        </Animated.View>
      )}
      <View style={{ flexDirection: 'row', alignItems: 'flex-end', gap: 8, paddingLeft: 10 }}>
        <TextInput
          ref={input}
          accessibilityLabel="Écrire un message"
          placeholder="Un avis sur ce résultat ?"
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
          maxLength={MAX}
          {...enter}
          onKeyPress={onKeyPress}
          // Under 16px, Safari on iPhone zooms the page in when the field gets the focus.
          style={{ flex: 1, minWidth: 0, minHeight: 40, maxHeight: 100, paddingVertical: 10, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
        />
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel="Envoyer"
          disabled={!ok}
          onPress={submit}
          style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: ok ? t.acc : t.card2 }}>
          <Send size={18} color={ok ? t.accTxt : t.mut} />
        </PressableScale>
      </View>
    </Card>
  );
}
