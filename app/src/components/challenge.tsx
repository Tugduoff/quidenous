import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Check, ChevronRight, Hourglass, Link2, Swords } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { Avatar, Card, Dialog, DialogTitle, PressableScale } from '@/components/ui';
import type { Round } from '@/lib/api';
import { useFeatures } from '@/lib/auth';
import { challengeRoom, communityError, communityKeys, type PodiumEntry, questionShareLink, useChallenges } from '@/lib/community';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useGroup, useRooms } from '@/lib/queries';
import { shareText } from '@/lib/share';
import { useTheme } from '@/lib/theme';

/** "Défier un autre salon" on a closed result: send it to another of my rooms, or share a link to play it. */
export function ChallengeButton({ round }: { round: Round }) {
  const t = useTheme();
  const tr = useT();
  const { community } = useFeatures();
  const [open, setOpen] = useState(false);
  if (!community || !round.closed_at) return null;
  return (
    <>
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={tr('community.challenge.action')}
        onPress={() => setOpen(true)}
        hitSlop={8}
        style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <Swords size={18} color={t.txt} />
      </PressableScale>
      <Dialog visible={open} onClose={() => setOpen(false)}>
        {open && <ChallengeDialog round={round} onDone={() => setOpen(false)} />}
      </Dialog>
    </>
  );
}

function ChallengeDialog({ round, onDone }: { round: Round; onDone: () => void }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const { data: rooms } = useRooms();
  const others = rooms?.filter((r) => r.id !== group?.id) ?? [];
  const [sent, setSent] = useState<Set<number>>(() => new Set());
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const send = useMutation({
    mutationFn: (roomId: number) => challengeRoom(round.id, roomId),
    onMutate: () => setError(null),
    onSuccess: (_, roomId) => {
      setSent((s) => new Set(s).add(roomId));
      qc.invalidateQueries({ queryKey: communityKeys.challenges(round.id) });
    },
    onError: (e) => setError(communityError(e)),
  });
  const link = useMutation({
    mutationFn: async () => {
      const url = await questionShareLink(round.id);
      return shareText(tr('community.challenge.share', { question: round.question, link: url }));
    },
    onMutate: () => setError(null),
    onSuccess: (outcome) => {
      if (outcome === 'copied') setCopied(true);
      else if (outcome === 'shared') onDone();
    },
    onError: (e) => setError(communityError(e)),
  });
  const row = { flexDirection: 'row', alignItems: 'center', gap: 12, height: 52, paddingHorizontal: 14, borderRadius: 16 } as const;
  return (
    <>
      <DialogTitle title={tr('community.challenge.action')} />
      {rooms && !others.length && <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>{tr('community.challenge.noRooms')}</Text>}
      {others.map((r) => {
        const done = sent.has(r.id);
        const busy = send.isPending && send.variables === r.id;
        return (
          <PressableScale
            key={r.id}
            scale={0.98}
            accessibilityRole="button"
            accessibilityLabel={done ? `${r.name} · ${tr('community.challenge.sent')}` : r.name}
            disabled={done || send.isPending}
            onPress={() => send.mutate(r.id)}
            style={[row, { backgroundColor: done ? t.accSoft : t.card2 }]}>
            <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>
              {r.name}
            </Text>
            {busy ? <ActivityIndicator color={t.mut} /> : done ? <Check size={18} color={t.accInk} /> : <ChevronRight size={18} color={t.mut} />}
          </PressableScale>
        );
      })}
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        disabled={link.isPending}
        onPress={() => link.mutate()}
        style={[row, { borderWidth: 1.5, borderColor: t.line }]}>
        {copied ? <Check size={18} color={t.accInk} /> : <Link2 size={18} color={t.txt} />}
        <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: copied ? t.accInk : t.txt }}>
          {tr(copied ? 'community.challenge.copied' : 'community.challenge.link')}
        </Text>
        {link.isPending && <ActivityIndicator color={t.mut} />}
      </PressableScale>
      {error && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag }}>{error}</Text>}
    </>
  );
}

/** Under a closed result: each room it was played against, side by side once both are over. Nothing when none. */
export function Challenges({ roundId }: { roundId: number }) {
  const t = useTheme();
  const tr = useT();
  const { data } = useChallenges(roundId);
  const { data: group } = useGroup();
  if (!data?.length || !group) return null;
  return (
    <View style={{ gap: 10 }}>
      {data.map((c, i) => (
        <Animated.View key={`${c.room}-${c.round_id ?? i}`} entering={motion.enter}>
          <Card style={{ padding: 16, gap: 14 }}>
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
              <Swords size={16} color={t.accInk} />
              <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>
                {tr('community.challenge.vs', { a: group.name, b: c.room })}
              </Text>
            </View>
            {c.podium && c.mine_podium ? (
              <View style={{ flexDirection: 'row', gap: 14 }}>
                <Podium title={group.name} entries={c.mine_podium} />
                <View style={{ width: 1, backgroundColor: t.line }} />
                <Podium title={c.room} entries={c.podium} />
              </View>
            ) : (
              <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                <Hourglass size={14} color={t.mut} />
                <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
                  {tr('community.challenge.waiting', { room: c.room })}
                </Text>
              </View>
            )}
          </Card>
        </Animated.View>
      ))}
    </View>
  );
}

/** One room's top 3, compact: face, name, score. */
function Podium({ title, entries }: { title: string; entries: PodiumEntry[] }) {
  const t = useTheme();
  return (
    <View style={{ flex: 1, minWidth: 0, gap: 8 }}>
      <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 11, letterSpacing: 0.6, color: t.mut, textTransform: 'uppercase' }}>
        {title}
      </Text>
      {entries.slice(0, 3).map((p, i) => (
        <View key={i} style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <Avatar member={{ id: -1 - i, name: p.name, color: p.color, photo_url: p.photo_url, avatar: p.avatar }} size={i === 0 ? 28 : 24} />
          <Text
            numberOfLines={1}
            style={{ flex: 1, fontFamily: i === 0 ? t.font.bold : t.font.semibold, fontSize: i === 0 ? 15 : 14, color: i === 0 ? t.txt : t.mut }}>
            {p.name}
          </Text>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: i === 0 ? t.accInk : t.mut }}>{p.score}</Text>
        </View>
      ))}
    </View>
  );
}
