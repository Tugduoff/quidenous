import { Check, Download, Heart } from 'lucide-react-native';
import { Text, View } from 'react-native';
import Animated, { type AnimatedStyle } from 'react-native-reanimated';
import type { ViewStyle } from 'react-native';
import { Stat } from '@/components/packs/pack-card';
import { useT } from '@/lib/i18n';
import type { PackDetail } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';

export function PackStatsRow({
  pack,
  installedHere,
  pop,
  onLike,
}: {
  pack: PackDetail;
  installedHere: boolean;
  pop: AnimatedStyle<ViewStyle>;
  onLike: (on: boolean) => void;
}) {
  const t = useTheme();
  const tr = useT();
  return (
  <View style={{ flexDirection: 'row', alignItems: 'center', gap: 14 }}>
    {pack.mine ? (
      <Stat icon={Heart} value={pack.likes} label={tr('packs.likes', { count: pack.likes })} />
    ) : (
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={tr('packs.like')}
        accessibilityState={{ selected: pack.liked }}
        onPress={() => onLike(!pack.liked)}
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          gap: 6,
          height: 36,
          paddingHorizontal: 14,
          borderRadius: t.radius.pill,
          backgroundColor: t.card,
        }}>
        <Animated.View style={pop}>
          <Heart size={17} color={pack.liked ? t.tag : t.txt} fill={pack.liked ? t.tag : 'transparent'} />
        </Animated.View>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{pack.likes}</Text>
      </PressableScale>
    )}
    <Stat icon={Download} value={pack.installs} label={tr('packs.installs', { count: pack.installs })} />
    <View style={{ flex: 1 }} />
    {installedHere && (
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
        <Check size={14} color={t.accInk} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.accInk }}>{tr('packs.installedHere')}</Text>
      </View>
    )}
  </View>
  );
}
