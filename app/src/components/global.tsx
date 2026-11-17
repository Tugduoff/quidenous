import { router } from 'expo-router';
import { ChartPie, Check, ChevronRight, Crown, Globe, type LucideIcon, Repeat, Sprout, UserRound, Users } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { formatNumber, useT } from '@/lib/i18n';
import { type GlobalFact, type GlobalFactId, useGlobal } from '@/lib/global';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

import { PressableScale } from './ui';

/** Home (Vote tab): today's global question in one line, tap to answer it or see the numbers. Nothing when the room has none. */
export function GlobalCard() {
  const t = useTheme();
  const tr = useT();
  const { data } = useGlobal();
  if (!data?.available) return null;
  const voted = data.my_vote != null;
  return (
    <Animated.View entering={motion.enter}>
      <PressableScale
        scale={0.98}
        accessibilityRole="button"
        accessibilityLabel={`${tr('globalQuestion.label')} : ${data.question}`}
        onPress={() => router.push('/global')}
        style={[
          {
            flexDirection: 'row',
            alignItems: 'center',
            gap: 12,
            paddingVertical: 10,
            paddingHorizontal: 12,
            borderRadius: 18,
            backgroundColor: t.card,
          },
          t.shadow,
        ]}>
        <View style={{ width: 32, height: 32, borderRadius: 16, alignItems: 'center', justifyContent: 'center', backgroundColor: t.accSoft }}>
          <Globe size={17} color={t.accInk} />
        </View>
        <View style={{ flex: 1, minWidth: 0, gap: 1 }}>
          <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 11, letterSpacing: 0.6, color: t.mut }}>
            {tr(voted ? 'globalQuestion.voted' : 'globalQuestion.label').toUpperCase()}
          </Text>
          <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
            {data.question}
          </Text>
        </View>
        {voted ? <Check size={18} color={t.accInk} /> : <ChevronRight size={18} color={t.mut} />}
      </PressableScale>
    </Animated.View>
  );
}

const ICONS: Record<GlobalFactId, LucideIcon> = {
  unanimous: Users,
  owner: Crown,
  newest: Sprout,
  last_winner: Repeat,
  self: UserRound,
  top_share: ChartPie,
};

/** Aggregates across rooms: an icon, the sentence with its percentage, a thin bar. */
export function FactRows({ facts }: { facts: GlobalFact[] }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 16 }}>
      {facts.map((f) => {
        const Icon = ICONS[f.id] ?? Globe;
        return (
          <View key={f.id} style={{ flexDirection: 'row', gap: 12 }}>
            <View style={{ width: 32, height: 32, borderRadius: 16, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
              <Icon size={16} color={t.accInk} />
            </View>
            <View style={{ flex: 1, gap: 8, paddingTop: 2 }}>
              <Text style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}>
                {tr(`globalQuestion.facts.${f.id}`, { pct: formatNumber(f.pct / 100, { style: 'percent' }) })}
              </Text>
              <View style={{ height: 4, borderRadius: 2, backgroundColor: t.card2, overflow: 'hidden' }}>
                <View style={{ width: `${Math.max(0, Math.min(100, f.pct))}%`, height: 4, borderRadius: 2, backgroundColor: t.acc }} />
              </View>
            </View>
          </View>
        );
      })}
    </View>
  );
}
