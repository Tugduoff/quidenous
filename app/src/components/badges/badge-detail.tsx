import { X } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { Mascot } from '@/components/mascot/mascot';
import { MedalDisc, TierPips, tierName, useTierInk } from '@/components/badges/title-chip';
import { BadgeProgress } from '@/components/badges/badge-progress';
import { showToast } from '@/lib/toast';
import { type Badge, useSetTitle } from '@/lib/api/badges';
import { formatNumber, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Button } from '@/components/ui/button';
import { Dialog } from '@/components/ui/dialog';
import { nextGoal } from '@/lib/utils/badges';

/** A tier's name in its metal. */
function TierLabel({ tier }: { tier: 1 | 2 | 3 }) {
  const t = useTheme();
  const tr = useT();
  return <Text style={{ fontFamily: t.font.bold, fontSize: 13, color: useTierInk(tier) }}>{tierName(tier, tr)}</Text>;
}

/** A badge up close: its family's mascot, tiers, what's next, and showing it as my title. */
export function BadgeDetail({ badge, shown, onClose }: { badge: Badge | null; shown: boolean; onClose: () => void }) {
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
              <BadgeProgress value={b.progress} goal={goal} height={6} />
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
