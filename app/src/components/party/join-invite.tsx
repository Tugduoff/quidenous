import { PartyPopper } from 'lucide-react-native';
import { ActivityIndicator, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { AvatarStack } from '@/components/ui/avatar';
import { Display } from '@/components/ui/text';
import type { Party } from '@/lib/api/party';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

/** The party a join link points at: its code, its name (or why it can't be joined), who is in. */
export function JoinInvite({ code, loading, closed, over, party }: { code: string; loading: boolean; closed: boolean; over: boolean; party: Party | undefined }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Animated.View entering={motion.question} style={{ gap: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <PartyPopper size={18} color={t.accInk} />
        <Text style={{ fontFamily: t.font.bold, fontSize: 15, letterSpacing: 2, color: t.accInk }}>{code}</Text>
      </View>
      {loading ? (
        <ActivityIndicator color={t.mut} style={{ alignSelf: 'flex-start' }} />
      ) : (
        <Display size={32}>{closed ? tr(over ? 'party.errors.over' : 'party.errors.notFound') : party?.name || tr('party.join.title')}</Display>
      )}
      {!closed && !!party?.players.length && (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          <AvatarStack members={party.players} size={28} max={6} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr('party.lobby.players', { count: party.players.length })}</Text>
        </View>
      )}
    </Animated.View>
  );
}
