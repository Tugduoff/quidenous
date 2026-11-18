import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Hourglass, Play } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { until } from '@/lib/utils/time';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { launchNext, type Group, groupKeys } from '@/lib/api/group';
import { questionKeys } from '@/lib/api/questions';
import { roundKeys } from '@/lib/api/rounds';
import { useNow } from '@/lib/hooks/use-now';

/**
 * Paced room, a closed question waiting for the next one (instead of "Suivante"): the owner's
 * "Lancer la suivante" when only they launch it, else when it opens by itself, or who it waits for.
 */
export function NextGate({ group, onLaunched }: { group: Group; onLaunched?: () => void }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { me } = useSession();
  const deadline = group.pace?.next_open_at ?? null;
  const now = useNow(deadline ? 30_000 : null);
  const owner = group.members.some((m) => m.id === me?.id && m.role === 'owner');
  const launch = useMutation({
    mutationFn: () => launchNext(group.id),
    onSuccess: () => onLaunched?.(),
    // The new question, the room's pacing and the queue it came from (not every query).
    onSettled: () => {
      qc.invalidateQueries({ queryKey: roundKeys.all });
      qc.invalidateQueries({ queryKey: groupKeys.all });
      qc.invalidateQueries({ queryKey: questionKeys.queue });
    },
  });
  const line = deadline ? tr('vote.gate.nextIn', { time: until(deadline, now) }) : tr('vote.gate.waitingOwner');
  const status = (
    <View accessibilityRole="text" style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 8, paddingVertical: 6 }}>
      <Hourglass size={16} color={t.mut} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{line}</Text>
    </View>
  );
  if (owner && group.settings.owner_next) {
    return (
      <Animated.View entering={motion.enter} style={{ gap: 4 }}>
        {deadline && status}
        <Button loading={launch.isPending} onPress={() => launch.mutate()}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{tr('vote.gate.launch')}</Text>
          <Play size={18} color={t.accTxt} fill={t.accTxt} />
        </Button>
      </Animated.View>
    );
  }
  return <Animated.View entering={motion.enter}>{status}</Animated.View>;
}
