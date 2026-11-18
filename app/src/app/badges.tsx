import { Check, X } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { Caps } from '@/components/stats/common';
import { MedalDisc, TierPips, TitleChip, tierName, useTierInk } from '@/components/badges/title-chip';
import { showToast } from '@/lib/toast';
import { useFeatures } from '@/lib/api/auth';
import { type Badge, chipOf, useBadges, useMyTitle, useSetTitle } from '@/lib/api/badges';
import { formatNumber, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Display } from '@/components/ui/text';
import { Dialog } from '@/components/ui/dialog';
import { BackButton } from '@/components/ui/back-button';

const GAP = 10;

/** The next threshold to reach (null once gold). */
const nextGoal = (b: Badge) => (b.tier < 3 ? (b.tiers[b.tier] ?? null) : null);

/** A slim bar: how far toward the next tier. */
function Progress({ value, goal, height = 4 }: { value: number; goal: number; height?: number }) {
  const t = useTheme();
  const share = goal > 0 ? Math.max(0, Math.min(1, value / goal)) : 0;
  return (
    <View style={{ alignSelf: 'stretch', height, borderRadius: height / 2, backgroundColor: t.card2, overflow: 'hidden' }}>
      <View style={{ width: `${share * 100}%`, height, borderRadius: height / 2, backgroundColor: t.accInk }} />
    </View>
  );
}

/** One medal of the grid. */
function Tile({ badge: b, width, shown, index, onOpen }: { badge: Badge; width: number; shown: boolean; index: number; onOpen: (b: Badge) => void }) {
  const t = useTheme();
  const tr = useT();
  const earned = b.tier > 0;
  const goal = nextGoal(b);
  const label = earned
    ? `${tr('badges.a11y.medal', { name: b.name, tier: tierName(b.tier as 1 | 2 | 3, tr) })}${shown ? `, ${tr('badges.a11y.displayed')}` : ''}`
    : tr('badges.a11y.medalLocked', { name: b.name, hint: b.hint });
  return (
    <Animated.View entering={motion.stagger(index)} style={{ width }}>
      <PressableScale
        scale={0.97}
        accessibilityRole="button"
        accessibilityLabel={label}
        onPress={() => onOpen(b)}
        style={[
          {
            alignItems: 'center',
            gap: 8,
            paddingVertical: 16,
            paddingHorizontal: 12,
            minHeight: 172,
            borderRadius: t.radius.card,
            backgroundColor: t.card,
            borderWidth: 2,
            borderColor: shown ? t.accInk : 'transparent',
          },
          t.shadow,
        ]}>
        <MedalDisc icon={b.icon} tier={b.tier} size={64} />
        <Text numberOfLines={2} style={{ textAlign: 'center', fontFamily: t.font.bold, fontSize: 14, lineHeight: 18, color: earned ? t.txt : t.mut }}>
          {b.name}
        </Text>
        {earned ? (
          <TierPips tier={b.tier} size={8} />
        ) : (
          <>
            <Text numberOfLines={2} style={{ textAlign: 'center', fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: t.mut }}>
              {b.hint}
            </Text>
            <View style={{ flex: 1 }} />
            {goal != null && <Progress value={b.progress} goal={goal} />}
          </>
        )}
        {shown && (
          <View
            style={{
              position: 'absolute',
              top: 8,
              right: 8,
              width: 22,
              height: 22,
              borderRadius: 11,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: t.acc,
            }}>
            <Check size={13} color={t.accTxt} strokeWidth={3} />
          </View>
        )}
      </PressableScale>
    </Animated.View>
  );
}

/** A tier's name in its metal. */
function TierLabel({ tier }: { tier: 1 | 2 | 3 }) {
  const t = useTheme();
  const tr = useT();
  return <Text style={{ fontFamily: t.font.bold, fontSize: 13, color: useTierInk(tier) }}>{tierName(tier, tr)}</Text>;
}

/** A badge up close: its family's mascot, tiers, what's next, and showing it as my title. */
function Detail({ badge, shown, onClose }: { badge: Badge | null; shown: boolean; onClose: () => void }) {
  const t = useTheme();
  const tr = useT();
  const setTitle = useSetTitle();
  // Keeps the badge on screen while the dialog fades out.
  const [last, setLast] = useState(badge);
  if (badge && badge !== last) setLast(badge);
  const b = badge ?? last;
  const pick = (key: string | null) => {
    onClose();
    setTitle.mutate(key, { onSuccess: () => showToast(tr(key ? 'badges.shownToast' : 'badges.hiddenToast')) });
  };
  const goal = b ? nextGoal(b) : null;
  return (
    <Dialog visible={!!badge} onClose={onClose}>
      {b && (
        <View style={{ gap: 14 }}>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={tr('common.close')}
            onPress={onClose}
            hitSlop={10}
            style={{
              position: 'absolute',
              top: 0,
              right: 0,
              zIndex: 1,
              width: 32,
              height: 32,
              borderRadius: 16,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: t.card2,
            }}>
            <X size={16} color={t.txt} />
          </Pressable>
          {/* The badge's family member: delighted at gold, pleased when earned, pondering while locked. */}
          <Mascot who={b.mascot} mood={b.tier === 3 ? 'celebrate' : b.tier > 0 ? 'happy' : 'think'} size={120} style={{ alignSelf: 'center' }} />
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
            <MedalDisc icon={b.icon} tier={b.tier} size={44} />
            <View style={{ flex: 1, gap: 4 }}>
              <Text accessibilityRole="header" style={{ fontFamily: t.font.display, fontSize: 24, lineHeight: 27, letterSpacing: -0.5, color: t.txt }}>
                {b.name}
              </Text>
              <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                <TierPips tier={b.tier} size={10} />
                {b.tier > 0 ? (
                  <TierLabel tier={b.tier as 1 | 2 | 3} />
                ) : (
                  <Text style={{ fontFamily: t.font.bold, fontSize: 13, color: t.mut }}>{tr('badges.locked')}</Text>
                )}
              </View>
            </View>
          </View>
          <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.txt }}>{b.hint}</Text>
          {goal != null ? (
            <View style={{ gap: 6 }}>
              <Progress value={b.progress} goal={goal} height={6} />
              <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, fontVariant: ['tabular-nums'] }}>
                {tr('badges.progress', { value: formatNumber(Math.min(b.progress, goal)), goal: formatNumber(goal) })}
              </Text>
            </View>
          ) : (
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('badges.maxed')}</Text>
          )}
          {b.tier > 0 &&
            (shown ? (
              <PressableScale
                accessibilityRole="button"
                onPress={() => pick(null)}
                style={{ height: 52, borderRadius: 999, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
                <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>{tr('badges.hide')}</Text>
              </PressableScale>
            ) : (
              <Button onPress={() => pick(b.key)}>{tr('badges.show')}</Button>
            ))}
        </View>
      )}
    </Dialog>
  );
}

/** My titles and badges: what I display, how many I have, every medal (earned or still to get). */
export default function BadgesScreen() {
  const t = useTheme();
  const tr = useT();
  const features = useFeatures();
  const query = useBadges();
  const { data } = query;
  const title = useMyTitle();
  const [open, setOpen] = useState<Badge | null>(null);
  const [width, setWidth] = useState(0);
  const cols = width >= 560 ? 3 : 2;
  const tile = width ? Math.floor((width - GAP * (cols - 1)) / cols) : 0;

  return (
    <Screen tabs={false}>
      <BackButton />
      <Display size={30}>{tr('badges.title')}</Display>
      {features.loaded && !features.badges ? (
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>{tr('badges.unavailable')}</Text>
      ) : (
        <QueryState query={query} />
      )}
      {data && (
        <>
          <Card style={{ padding: 16, gap: 12 }}>
            <View style={{ gap: 8 }}>
              <Caps size={11}>{tr('badges.shown')}</Caps>
              {title ? (
                <TitleChip chip={title} size="md" />
              ) : (
                <View style={{ gap: 2 }}>
                  <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>{tr('badges.noTitle')}</Text>
                  <Text style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>{tr('badges.noTitleHint')}</Text>
                </View>
              )}
            </View>
            <View style={{ gap: 6 }}>
              <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>
                {tr('badges.count', { earned: data.earned, total: data.total })}
              </Text>
              <Progress value={data.earned} goal={data.total} height={6} />
            </View>
          </Card>
          <View onLayout={(e) => setWidth(e.nativeEvent.layout.width)} style={{ flexDirection: 'row', flexWrap: 'wrap', gap: GAP }}>
            {tile > 0 &&
              data.badges.map((b, i) => (
                <Tile key={b.key} badge={b} width={tile} index={i} shown={data.title === b.key && !!chipOf(b)} onOpen={setOpen} />
              ))}
          </View>
        </>
      )}
      <Detail badge={open} shown={!!open && data?.title === open.key} onClose={() => setOpen(null)} />
    </Screen>
  );
}
