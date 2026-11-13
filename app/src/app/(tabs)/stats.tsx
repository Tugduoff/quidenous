import { router } from 'expo-router';
import { useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Awards } from '@/components/stats/awards';
import { Section } from '@/components/stats/common';
import { Heatmap } from '@/components/stats/heatmap';
import { KeyNumbers } from '@/components/stats/key-numbers';
import { PeriodSwitch } from '@/components/stats/period-switch';
import { Podium, podiumOf } from '@/components/stats/podium';
import { ProfileSheet } from '@/components/stats/profile-sheet';
import { Sparkline } from '@/components/stats/sparkline';
import { StoryCarousel } from '@/components/stats/story-carousel';
import { VoteMatrix } from '@/components/stats/vote-matrix';
import { Display } from '@/components/ui';
import type { StatsPeriod, Story } from '@/lib/api';
import { type TKey, useT } from '@/lib/i18n';
import { useMembers, useStats } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const EMPTY: Record<StatsPeriod, TKey> = {
  '7d': 'stats.empty.week',
  '30d': 'stats.empty.month',
  all: 'stats.empty.all',
};

export default function StatsScreen() {
  const t = useTheme();
  const tr = useT();
  const [period, setPeriod] = useState<StatsPeriod>('30d');
  const { data: s, isLoading, isPlaceholderData } = useStats(period);
  const { byId } = useMembers();
  const [profileId, setProfileId] = useState<number | null>(null);

  const header = (
    <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
      <Display size={30}>{tr('stats.title')}</Display>
      <PeriodSwitch value={period} onChange={setPeriod} />
    </View>
  );

  if (!s) {
    return (
      <Screen>
        {header}
        {isLoading && <ActivityIndicator color={t.mut} style={{ marginTop: 40 }} />}
      </Screen>
    );
  }
  if (s.questions === 0) {
    return (
      <Screen>
        {header}
        <Text style={{ marginTop: 40, textAlign: 'center', fontFamily: t.font.semibold, fontSize: 16, lineHeight: 22, color: t.mut }}>
          {tr(EMPTY[period])}
        </Text>
      </Screen>
    );
  }

  const name = (id: number) => byId.get(id)?.name ?? '?';
  const profiles = new Map(s.profiles.map((p) => [p.member_id, p]));
  // Only people with a profile open one (an older server has none).
  const open = (id: number) => profiles.has(id) && setProfileId(id);
  const openStory = (story: Story) => {
    if (story.round_id != null) router.push(`/round/${story.round_id}`);
    else if (story.member_ids[0] != null) open(story.member_ids[0]);
  };

  const podium = podiumOf(s.profiles);
  // The matrix lists current members (profiles), or whoever shows up in the votes on a server without profiles.
  const ids = s.profiles.length
    ? s.profiles.map((p) => p.member_id)
    : [...new Set(s.edges.flatMap((e) => [e.voter_id, e.target_id]))].filter((id) => byId.has(id) && !byId.get(id)?.left).sort((a, b) => a - b);
  const edges = s.edges.filter((e) => ids.includes(e.voter_id) && ids.includes(e.target_id));
  const heat = s.heatmap.some((row) => row.some((v) => v > 0));
  const activity = s.activity && s.activity.questions.length > 1 && s.activity.questions.some((v) => v > 0) ? s.activity : null;

  return (
    <Screen>
      {header}
      <View style={{ gap: 36, paddingBottom: 24, opacity: isPlaceholderData ? 0.5 : 1 }}>
        {s.stories.length > 0 && <StoryCarousel key={period} stories={s.stories} period={period} byId={byId} onOpen={openStory} />}
        <KeyNumbers s={s} />
        {podium.length > 0 && (
          <Section title={tr('stats.section.podium')}>
            <Podium top={podium} byId={byId} onOpen={open} />
          </Section>
        )}
        <Awards s={s} byId={byId} name={name} onOpen={open} />
        {ids.length >= 2 && edges.length > 0 && (
          <Section title={tr('stats.section.matrix')}>
            <VoteMatrix ids={ids} edges={edges} byId={byId} onOpen={open} />
          </Section>
        )}
        {heat && (
          <Section title={tr('stats.section.heatmap')}>
            <Heatmap data={s.heatmap} />
          </Section>
        )}
        {activity && (
          <Section title={tr('stats.section.rhythm')}>
            <Sparkline activity={activity} />
          </Section>
        )}
      </View>
      <ProfileSheet
        profile={profileId != null ? (profiles.get(profileId) ?? null) : null}
        byId={byId}
        onOpen={open}
        onClose={() => setProfileId(null)}
      />
    </Screen>
  );
}
