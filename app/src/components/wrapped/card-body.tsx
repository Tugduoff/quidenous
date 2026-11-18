import { Text, View } from 'react-native';
import Animated, { FadeIn, FadeInDown, ZoomIn } from 'react-native-reanimated';
import { Mascot } from '@/components/mascot/mascot';
import { Caps, NamedIcon } from '@/components/stats/common';
import { useT } from '@/lib/i18n';
import { LAYOUT_EASE } from '@/lib/motion';
import { useTheme } from '@/lib/theme';
import type { WrappedCard } from '@/lib/api/wrapped';
import { Avatar } from '@/components/ui/avatar';
import type { Member } from '@/lib/api/group';
import { bigSize } from '@/lib/utils/wrapped';
import { INK, sys } from '@/components/wrapped/constants';

const asMember = (p: WrappedCard['people'][number], i: number): Member => ({
  id: -1 - i,
  name: p.name,
  color: p.color,
  photo_url: p.photo_url,
  avatar: p.avatar,
});

export function CardBody({ card, n, total }: { card: WrappedCard; n: number; total: number }) {
  const t = useTheme();
  const tr = useT();
  const big = bigSize(card.big);
  const label = [tr('wrapped.page', { n, total }), card.title, card.question, card.big, card.line].filter(Boolean).join('. ');
  return (
    <View accessible accessibilityLabel={label} style={{ flex: 1, paddingHorizontal: 24, paddingTop: 12, paddingBottom: 8 }}>
      <Animated.View entering={FadeIn.duration(260).reduceMotion(sys)} style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <NamedIcon name={card.icon} size={18} color={INK} strokeWidth={2.5} />
        <Caps color={INK} size={13} lines={2}>
          {card.title}
        </Caps>
      </Animated.View>
      <View style={{ flex: 1, alignItems: 'flex-end', justifyContent: 'center' }}>
        {card.mascot && (
          <Animated.View entering={ZoomIn.springify().damping(14).stiffness(200).delay(120).reduceMotion(sys)}>
            <Mascot who={card.mascot.who} mood={card.mascot.mood} size={168} />
          </Animated.View>
        )}
      </View>
      {card.people.length > 0 && (
        <Animated.View entering={FadeInDown.duration(320).delay(80).easing(LAYOUT_EASE).reduceMotion(sys)} style={{ flexDirection: 'row', marginBottom: 10 }}>
          {card.people.slice(0, 2).map((p, i) => (
            <View key={i} style={{ marginLeft: i ? -16 : 0, borderRadius: 40, borderWidth: 3, borderColor: INK }}>
              <Avatar member={asMember(p, i)} size={64} />
            </View>
          ))}
        </Animated.View>
      )}
      {!!card.question && (
        <Animated.View
          entering={FadeInDown.duration(320).delay(60).easing(LAYOUT_EASE).reduceMotion(sys)}
          style={{ marginBottom: 12, padding: 14, borderRadius: 18, backgroundColor: '#FFFFFF8C' }}>
          <Text numberOfLines={4} style={{ fontFamily: t.font.semibold, fontSize: 17, lineHeight: 23, color: INK }}>
            {card.question}
          </Text>
        </Animated.View>
      )}
      <Animated.View entering={FadeInDown.duration(380).delay(100).easing(LAYOUT_EASE).reduceMotion(sys)}>
        <Text
          numberOfLines={2}
          adjustsFontSizeToFit
          minimumFontScale={0.5}
          style={{ fontFamily: t.font.display, fontSize: big, lineHeight: big * 1.04, letterSpacing: big > 60 ? -3 : -1.5, color: INK }}>
          {card.big}
        </Text>
      </Animated.View>
      <Animated.View entering={FadeIn.duration(320).delay(240).reduceMotion(sys)}>
        <Text numberOfLines={4} style={{ marginTop: 6, fontFamily: t.font.semibold, fontSize: 20, lineHeight: 27, color: INK }}>
          {card.line}
        </Text>
      </Animated.View>
    </View>
  );
}
