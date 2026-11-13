import { Check, Share2 } from 'lucide-react-native';
import { useRef, useState } from 'react';
import { ActivityIndicator, Pressable, ScrollView, type ScrollViewInstance, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import Svg, { Defs, LinearGradient, Rect, Stop } from 'react-native-svg';

import { Avatar, PressableScale } from '@/components/ui';
import { type Member, photoUrl, type StatsPeriod, type Story } from '@/lib/api';
import { parseAvatar } from '@/lib/avatar';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { shareStory } from '@/lib/share';
import { useTheme } from '@/lib/theme';

import { AWARD_ICONS, awardTitle, Caps, iconFor } from './common';

const HEIGHT = 300;
/** Text on a member's colour (they are all light and bright), same as the initials on avatars. */
const INK = '#121216';

const bigSize = (s: string) => (s.length <= 3 ? 76 : s.length <= 6 ? 60 : s.length <= 10 ? 46 : 36);

function StoryCard({
  story,
  period,
  byId,
  onOpen,
}: {
  story: Story;
  period: StatsPeriod;
  byId: Map<number, Member>;
  onOpen: (story: Story) => void;
}) {
  const t = useTheme();
  const tr = useT();
  // Known awards get their title in the app's language; anything newer keeps the server's.
  const title = AWARD_ICONS[story.key] ? awardTitle(story.key) : story.title;
  const people = story.member_ids.map((id) => byId.get(id)).filter((m): m is Member => !!m);
  const [a, b] = [people[0]?.color ?? t.acc, people[1]?.color ?? people[0]?.color ?? t.acc];
  const ink = people.length ? INK : t.accTxt;
  const Icon = iconFor(story.icon);
  const [state, setState] = useState<'idle' | 'busy' | 'copied'>('idle');
  // Measured: on Android a percentage-sized Svg resolves against the padded box and leaves a band uncovered.
  const [box, setBox] = useState({ w: 0, h: 0 });
  const share = async () => {
    setState('busy');
    const outcome = await shareStory(period, { ...story, title });
    setState(outcome === 'copied' ? 'copied' : 'idle');
    if (outcome === 'copied') setTimeout(() => setState('idle'), 1800);
  };
  return (
    <PressableScale
      scale={0.98}
      accessibilityRole="button"
      accessibilityLabel={`${title}. ${story.big}. ${story.line}`}
      onPress={() => onOpen(story)}
      onLayout={(e) => setBox({ w: e.nativeEvent.layout.width, h: e.nativeEvent.layout.height })}
      style={{ height: HEIGHT, borderRadius: 28, overflow: 'hidden', padding: 22, backgroundColor: a }}>
      {/* Two people: their colours blend across the card. */}
      {a !== b && box.w > 0 && (
        <Svg style={{ position: 'absolute', top: 0, left: 0 }} width={box.w} height={box.h}>
          <Defs>
            <LinearGradient id={`g-${story.key}`} x1="0" y1="0" x2="1" y2="1">
              <Stop offset="0" stopColor={a} />
              <Stop offset="1" stopColor={b} />
            </LinearGradient>
          </Defs>
          <Rect x="0" y="0" width={box.w} height={box.h} fill={`url(#g-${story.key})`} />
        </Svg>
      )}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <Icon size={16} color={ink} strokeWidth={2.5} />
        <View style={{ flex: 1 }}>
          <Caps color={ink}>{title}</Caps>
        </View>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={state === 'copied' ? tr('stats.story.linkCopied') : tr('common.share')}
          onPress={share}
          disabled={state === 'busy'}
          hitSlop={10}
          style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: '#1212161F' }}>
          {state === 'busy' ? (
            <ActivityIndicator size="small" color={ink} />
          ) : state === 'copied' ? (
            <Check size={17} color={ink} />
          ) : (
            <Share2 size={17} color={ink} />
          )}
        </Pressable>
      </View>
      <View style={{ flex: 1 }} />
      {people.length > 0 && (
        <View style={{ flexDirection: 'row', marginBottom: 6 }}>
          {people.slice(0, 2).map((m, i) => (
            <View key={m.id} style={{ marginLeft: i ? -14 : 0, borderRadius: 40, borderWidth: 3, borderColor: i ? b : a }}>
              {/* On their own colour a plain avatar would vanish: initials go ink-on-colour reversed. */}
              {photoUrl(m) || parseAvatar(m.avatar) ? (
                <Avatar member={m} size={56} />
              ) : (
                <View style={{ width: 56, height: 56, borderRadius: 28, alignItems: 'center', justifyContent: 'center', backgroundColor: INK }}>
                  <Text style={{ fontFamily: t.font.bold, fontSize: 24, color: m.color }}>{m.name[0]?.toUpperCase()}</Text>
                </View>
              )}
            </View>
          ))}
        </View>
      )}
      <Text
        numberOfLines={1}
        adjustsFontSizeToFit
        minimumFontScale={0.5}
        style={{ fontFamily: t.font.display, fontSize: bigSize(story.big), lineHeight: bigSize(story.big) * 1.1, letterSpacing: -2, color: ink }}>
        {story.big}
      </Text>
      <Text numberOfLines={3} style={{ fontFamily: t.font.semibold, fontSize: 17, lineHeight: 23, color: ink }}>
        {story.line}
      </Text>
    </PressableScale>
  );
}

/** The period's highlights, one card per page, swiped sideways. Bleeds to the screen edges (the screen pads by 20). */
export function StoryCarousel({
  stories,
  period,
  byId,
  onOpen,
}: {
  stories: Story[];
  period: StatsPeriod;
  byId: Map<number, Member>;
  onOpen: (story: Story) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const [width, setWidth] = useState(0);
  const [page, setPage] = useState(0);
  const ref = useRef<ScrollViewInstance>(null);
  const current = Math.min(page, stories.length - 1);
  return (
    <Animated.View entering={motion.question} style={{ marginHorizontal: -20, gap: 12 }} onLayout={(e) => setWidth(e.nativeEvent.layout.width)}>
      {width > 0 && (
        <ScrollView
          ref={ref}
          horizontal
          pagingEnabled
          showsHorizontalScrollIndicator={false}
          scrollEventThrottle={32}
          onScroll={(e) => setPage(Math.round(e.nativeEvent.contentOffset.x / width))}>
          {stories.map((s) => (
            <View key={s.key} style={{ width, paddingHorizontal: 20 }}>
              <StoryCard story={s} period={period} byId={byId} onOpen={onOpen} />
            </View>
          ))}
        </ScrollView>
      )}
      {stories.length > 1 && (
        <View style={{ flexDirection: 'row', justifyContent: 'center', gap: 6 }}>
          {stories.map((s, i) => (
            <Pressable
              key={s.key}
              accessibilityRole="button"
              accessibilityLabel={tr('stats.story.page', { n: i + 1, total: stories.length })}
              hitSlop={6}
              onPress={() => ref.current?.scrollTo({ x: i * width, animated: true })}>
              <Animated.View
                layout={motion.layout}
                style={{ width: i === current ? 18 : 6, height: 6, borderRadius: 3, backgroundColor: i === current ? t.accInk : t.line }}
              />
            </Pressable>
          ))}
        </View>
      )}
    </Animated.View>
  );
}
