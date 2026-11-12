import { router } from 'expo-router';
import { ChevronRight, X } from 'lucide-react-native';
import { type ReactNode, useRef } from 'react';
import { Pressable, ScrollView, Text, useWindowDimensions, View } from 'react-native';

import { Avatar, Dialog, PressableScale, QuestionText } from '@/components/ui';
import type { Member, MemberStats, QuestionRef } from '@/lib/api';
import { useTheme } from '@/lib/theme';

import { AWARD_META, Caps, dec, duration, iconFor, int, plural } from './common';

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
          <Text style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut, width: 34 }}>{plural(p.value, 'vote', 'votes')}</Text>
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
                {p.present} {plural(p.present, 'question jouée', 'questions jouées')}
              </Text>
            </View>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel="Fermer"
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

          {p.titles.some((k) => AWARD_META[k]) && (
            <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 6 }}>
              {p.titles
                .filter((k) => AWARD_META[k])
                .map((k) => {
                  const Icon = iconFor(AWARD_META[k].icon);
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
                        {AWARD_META[k].title}
                      </Text>
                    </View>
                  );
                })}
            </View>
          )}

          <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8 }}>
            <Mini value={`${Math.round(p.win_rate * 100)} %`} label={`${p.wins} ${plural(p.wins, 'victoire', 'victoires')}`} />
            <Mini value={int(p.received)} label={plural(p.received, 'vote reçu', 'votes reçus')} />
            <Mini value={int(p.votes_cast)} label={plural(p.votes_cast, 'vote donné', 'votes donnés')} />
            <Mini value={int(p.messages)} label={plural(p.messages, 'message', 'messages')} />
            {p.median_vote_seconds != null && <Mini value={duration(p.median_vote_seconds)} label="pour voter" />}
            {p.avg_rank != null && <Mini value={`${dec(p.avg_rank)}e`} label="place moyenne" />}
            {p.self_votes > 0 && <Mini value={int(p.self_votes)} label="votes pour soi" />}
          </View>

          {p.top_voters.length > 0 && (
            <Block title="Ses fans">
              <People list={p.top_voters} byId={byId} onOpen={onOpen} />
            </Block>
          )}
          {p.top_targets.length > 0 && (
            <Block title="Son cœur balance">
              <People list={p.top_targets} byId={byId} onOpen={onOpen} />
            </Block>
          )}
          {p.best && (
            <Block title="Son heure de gloire">
              <Question
                q={p.best}
                line={`${p.best.value} ${plural(p.best.value, 'vote', 'votes')} sur ${p.best.total}`}
                onPress={() => openRound(p.best!.round_id)}
              />
            </Block>
          )}
          {p.worst && (
            <Block title="Son pire classement">
              <Question q={p.worst} line={`${p.worst.value}e place en moyenne`} onPress={() => openRound(p.worst!.round_id)} />
            </Block>
          )}
        </ScrollView>
      )}
    </Dialog>
  );
}
