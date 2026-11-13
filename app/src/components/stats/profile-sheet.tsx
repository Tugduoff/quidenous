import { router } from 'expo-router';
import { ChevronRight, X } from 'lucide-react-native';
import { type ReactNode, useRef } from 'react';
import { Pressable, ScrollView, Text, useWindowDimensions, View } from 'react-native';

import { Avatar, Dialog, PressableScale, QuestionText } from '@/components/ui';
import type { Member, MemberStats, QuestionRef } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

import { AWARD_ICONS, awardTitle, Caps, duration, iconFor, int, pct, place } from './common';

function Mini({ value, label }: { value: string; label: string }) {
  const t = useTheme();
  return (
    <View style={{ flexBasis: '45%', flexGrow: 1, padding: 12, borderRadius: 16, backgroundColor: t.card2, gap: 2 }}>
      <Text numberOfLines={1} style={{ fontFamily: t.font.display, fontSize: 22, letterSpacing: -0.5, color: t.txt }}>
        {value}
      </Text>
      <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>
        {label}
      </Text>
    </View>
  );
}

function Block({ title, children }: { title: string; children: ReactNode }) {
  return (
    <View style={{ gap: 8 }}>
      <Caps size={11}>{title}</Caps>
      {children}
    </View>
  );
}

function People({ list, byId, onOpen }: { list: { member_id: number; value: number }[]; byId: Map<number, Member>; onOpen: (id: number) => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ gap: 6 }}>
      {list.map((p) => (
        <Pressable
          key={p.member_id}
          accessibilityRole="button"
          onPress={() => onOpen(p.member_id)}
          style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 2 }}>
          <Avatar member={byId.get(p.member_id)} size={30} />
          <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
            {byId.get(p.member_id)?.name ?? '?'}
          </Text>
          <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.txt }}>{p.value}</Text>
          <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut, width: 34 }}>{tr('stats.unit.votes', { count: p.value })}</Text>
        </Pressable>
      ))}
    </View>
  );
}

function Question({ q, line, onPress }: { q: QuestionRef; line: string; onPress: () => void }) {
  const t = useTheme();
  return (
    <PressableScale
      scale={0.98}
      accessibilityRole="button"
      onPress={onPress}
      style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 12, borderRadius: 16, backgroundColor: t.card2 }}>
      <View style={{ flex: 1, gap: 4 }}>
        <QuestionText
          text={q.question}
          rank={q.kind === 'rank'}
          style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}
        />
        <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>{line}</Text>
      </View>
      <ChevronRight size={16} color={t.mut} />
    </PressableScale>
  );
}

/** One person's period: their titles, numbers, fans, crushes, best and worst moments. Tapping someone else switches to them. */
export function ProfileSheet({
  profile,
  byId,
  onOpen,
  onClose,
}: {
  profile: MemberStats | null;
  byId: Map<number, Member>;
  onOpen: (id: number) => void;
  onClose: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  const { height } = useWindowDimensions();
  // Keeps the last person on screen while the dialog fades out.
  const last = useRef(profile);
  if (profile) last.current = profile;
  const p = last.current;
  const m = p ? byId.get(p.member_id) : undefined;
  const openRound = (id: number) => {
    onClose();
    router.push(`/round/${id}`);
  };
  return (
    <Dialog visible={!!profile} onClose={onClose}>
      {p && (
        <ScrollView
          key={p.member_id}
          style={{ maxHeight: height * 0.72, margin: -4 }}
          contentContainerStyle={{ padding: 4, gap: 20 }}
          showsVerticalScrollIndicator={false}>
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 14 }}>
            <Avatar member={m} size={60} />
            <View style={{ flex: 1, gap: 2 }}>
              <Text numberOfLines={1} style={{ fontFamily: t.font.display, fontSize: 24, letterSpacing: -0.5, color: t.txt }}>
                {m?.name ?? '?'}
              </Text>
              <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
                {tr('stats.profile.played', { count: p.present })}
              </Text>
            </View>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={tr('common.close')}
              onPress={onClose}
              hitSlop={10}
              style={{
                alignSelf: 'flex-start',
                width: 32,
                height: 32,
                borderRadius: 16,
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: t.card2,
              }}>
              <X size={16} color={t.txt} />
            </Pressable>
          </View>

          {p.titles.some((k) => AWARD_ICONS[k]) && (
            <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 6 }}>
              {p.titles
                .filter((k) => AWARD_ICONS[k])
                .map((k) => {
                  const Icon = iconFor(AWARD_ICONS[k]);
                  return (
                    <View
                      key={k}
                      style={{
                        flexDirection: 'row',
                        alignItems: 'center',
                        gap: 6,
                        height: 30,
                        paddingHorizontal: 10,
                        borderRadius: t.radius.pill,
                        backgroundColor: t.accSoft,
                      }}>
                      <Icon size={13} color={t.accInk} strokeWidth={2.5} />
                      <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 12, color: t.txt }}>
                        {awardTitle(k)}
                      </Text>
                    </View>
                  );
                })}
            </View>
          )}

          <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8 }}>
            <Mini value={pct(p.win_rate, 1)} label={tr('stats.profile.wins', { count: p.wins })} />
            <Mini value={int(p.received)} label={tr('stats.unit.received', { count: p.received })} />
            <Mini value={int(p.votes_cast)} label={tr('stats.unit.cast', { count: p.votes_cast })} />
            <Mini value={int(p.messages)} label={tr('stats.unit.messages', { count: p.messages })} />
            {p.median_vote_seconds != null && <Mini value={duration(p.median_vote_seconds)} label={tr('stats.profile.toVote')} />}
            {p.avg_rank != null && <Mini value={place(p.avg_rank)} label={tr('stats.profile.avgPlace')} />}
            {p.self_votes > 0 && <Mini value={int(p.self_votes)} label={tr('stats.profile.selfVotes')} />}
          </View>

          {p.top_voters.length > 0 && (
            <Block title={tr('stats.profile.fans')}>
              <People list={p.top_voters} byId={byId} onOpen={onOpen} />
            </Block>
          )}
          {p.top_targets.length > 0 && (
            <Block title={tr('stats.profile.crushes')}>
              <People list={p.top_targets} byId={byId} onOpen={onOpen} />
            </Block>
          )}
          {p.best && (
            <Block title={tr('stats.profile.best')}>
              <Question
                q={p.best}
                line={tr('stats.profile.bestLine', { count: p.best.value, total: p.best.total })}
                onPress={() => openRound(p.best!.round_id)}
              />
            </Block>
          )}
          {p.worst && (
            <Block title={tr('stats.profile.worst')}>
              <Question q={p.worst} line={tr('stats.profile.worstLine', { n: p.worst.value })} onPress={() => openRound(p.worst!.round_id)} />
            </Block>
          )}
        </ScrollView>
      )}
    </Dialog>
  );
}
