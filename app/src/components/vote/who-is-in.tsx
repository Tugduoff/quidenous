import { Moon, SkipForward } from 'lucide-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { MemberGrid, MemberTile } from '@/components/vote/member-grid';
import { Mascot } from '@/components/mascot/mascot';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { AvatarStack } from '@/components/ui/avatar';
import { Dialog } from '@/components/ui/dialog';
import { type Member, useGroup } from '@/lib/api/group';
import { type Round } from '@/lib/api/rounds';
import { PendingRow } from '@/components/vote/pending-row';
import { VoteProgress } from '@/components/vote/vote-progress';

/** Up to this many people still to vote, one row each; more, a grid of faces. */
const BIG_WAIT = 5;

/**
 * Who's in, who we're waiting for (`done`: who voted, or for a said question who wrote). A card (not Card) that animates
 * its height as people leave the list; each pending person with the skip and the bell.
 */
export function WhoIsIn({
  round,
  members,
  byId,
  done,
  title,
}: {
  round: Round;
  members: Member[];
  byId: Map<number, Member>;
  done: number[];
  title: string;
}) {
  const t = useTheme();
  const tr = useT();
  const pending = members.filter((m) => !done.includes(m.id));
  // The face tapped in the grid; closes by itself once they've voted.
  const [open, setOpen] = useState<number | null>(null);
  const opened = pending.find((m) => m.id === open);
  // With two people a majority is one person deciding alone: no skip vote then (nor when the owner turned it off).
  const group = useGroup();
  const canSkip = members.length >= 3 && group.data?.settings.skip_vote !== false;
  const label = { fontFamily: t.font.bold, fontSize: 11, letterSpacing: 0.6, color: t.mut } as const;
  return (
    <>
      {/* Who's in, who we're waiting for. A card (not Card) that animates its height as people leave the list. */}
      <Animated.View layout={motion.layout} style={[{ backgroundColor: t.card, borderRadius: t.radius.card, padding: 16, gap: 14 }, t.shadow]}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          {/* Roupi dozes while the others take their time (not when there's a whole grid of faces to show). */}
          {pending.length > 0 && pending.length <= BIG_WAIT && (
            <Mascot who="roupi" mood="sleepy" size={56} style={{ marginVertical: -8, marginLeft: -6 }} />
          )}
          <Text style={{ flex: 1, fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>{title}</Text>
          <AvatarStack members={done.map((id) => byId.get(id))} size={26} ring={t.card} max={6} />
        </View>
        <VoteProgress value={done.length / Math.max(round.member_count, 1)} />
        {pending.length > 0 && <Text style={label}>{tr('vote.waiting.pending')}</Text>}
        {/* Many still to vote: a grid of faces; tap one for the bell and the skip. */}
        {pending.length > BIG_WAIT ? (
          <MemberGrid>
            {pending.map((m) => {
              const skipped = round.skipped_ids.includes(m.id);
              const away = !!round.away_ids?.includes(m.id);
              const asked = round.skips.some((s) => s.target_id === m.id);
              return (
                <Animated.View key={m.id} exiting={motion.exit} layout={motion.layout} style={{ width: '20%' }}>
                  <MemberTile
                    member={m}
                    cols={1}
                    plain
                    dim={skipped || away}
                    badge={skipped || asked ? <SkipForward size={12} color={t.mut} /> : away ? <Moon size={12} color={t.mut} /> : undefined}
                    label={
                      skipped ? tr('vote.waiting.skippedLabel', { name: m.name }) : away ? tr('vote.waiting.awayLabel', { name: m.name }) : m.name
                    }
                    onPress={() => setOpen(m.id)}
                  />
                </Animated.View>
              );
            })}
          </MemberGrid>
        ) : (
          /* Someone who votes fades out of the list and the others close up. */
          pending.map((m) => <PendingRow key={m.id} member={m} round={round} canSkip={canSkip} />)
        )}
      </Animated.View>
      <Dialog visible={!!opened} onClose={() => setOpen(null)}>
        {opened && <PendingRow member={opened} round={round} canSkip={canSkip} big />}
      </Dialog>
    </>
  );
}
