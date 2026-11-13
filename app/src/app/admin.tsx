import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Check, X } from 'lucide-react-native';
import { useState } from 'react';
import { Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { MiniTag, PageHeader } from '@/components/pack-card';
import { Screen } from '@/components/screen';
import { Card, ConfirmModal, Display, PressableScale, QuestionText } from '@/components/ui';
import { api } from '@/lib/api';
import { dateFormat, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { type PackTag, packKeys, REPORT_REASONS, type ReportReason, type Review, TAG_ICONS, useCommunityMe, useReview } from '@/lib/packs';
import { useTheme } from '@/lib/theme';

/** The built-in deck's tones a suggestion can join (none picked: the server files it under `potes`). */
const DECK_TAGS: PackTag[] = ['potes', 'pimente', 'boulot', 'famille'];
const isReason = (r: string | null): r is ReportReason => !!r && (REPORT_REASONS as string[]).includes(r);

/** Admins: packs sent to review by reports (approve or remove with a reason), suggestions voted up (approve or reject). */
export default function AdminScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { data: me } = useCommunityMe();
  const { data: review } = useReview(!!me?.admin);
  const [removing, setRemoving] = useState<number | null>(null);
  const [reason, setReason] = useState('');

  const act = useMutation({
    mutationFn: ({ path, json }: { path: string; json?: unknown; drop: { pack?: number; suggestion?: number } }) =>
      api(`/admin${path}`, { method: 'POST', json: json ?? {} }),
    onSuccess: (_, { drop }) => {
      qc.setQueryData<Review>(
        packKeys.review,
        (r) => r && { packs: r.packs.filter((p) => p.id !== drop.pack), suggestions: r.suggestions.filter((s) => s.id !== drop.suggestion) },
      );
      if (drop.pack) qc.invalidateQueries({ queryKey: packKeys.all });
      setRemoving(null);
      setReason('');
    },
  });
  const empty = review && !review.packs.length && !review.suggestions.length;
  const section = { fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4, marginTop: 4 };
  const day = dateFormat({ day: 'numeric', month: 'short' });

  return (
    <Screen tabs={false}>
      <PageHeader fallback="/packs" />
      <Display size={30}>{tr('admin.title')}</Display>
      {(empty || me?.admin === false) && (
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut, textAlign: 'center', marginTop: 24 }}>{tr('admin.empty')}</Text>
      )}
      {act.isError && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag }}>{tr('common.failed')}</Text>}

      {!!review?.packs.length && <Text style={section}>{tr('admin.packs')}</Text>}
      {review?.packs.map((p, i) => (
        <Animated.View key={`p${p.id}`} entering={motion.stagger(i)} exiting={motion.exit} layout={motion.layout}>
          <Card style={{ padding: 16, gap: 8 }}>
            <PressableScale scale={0.98} accessibilityRole="button" onPress={() => router.push(`/pack/${p.id}`)} style={{ gap: 2 }}>
              <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
                  {p.name}
                </Text>
                {p.spicy && <MiniTag label={tr('packs.spicy')} tone="hot" />}
              </View>
              <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
                {tr('packs.byline', { count: p.count, author: p.author })}
              </Text>
            </PressableScale>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.tag }}>{tr('admin.reports', { count: p.reports.length })}</Text>
            {p.reports.map((r, j) => (
              <Text key={j} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.txt }}>
                <Text style={{ fontFamily: t.font.semibold }}>{isReason(r.reason) ? tr(`packs.reasons.${r.reason}`) : '—'}</Text>
                {r.details ? ` · ${r.details}` : ''}
                <Text style={{ color: t.mut }}>{` · ${day.format(new Date(r.created_at))}`}</Text>
              </Text>
            ))}
            <Actions
              busy={act.isPending}
              onReject={() => setRemoving(p.id)}
              rejectLabel={tr('admin.remove')}
              onApprove={() => act.mutate({ path: `/packs/${p.id}/approve`, drop: { pack: p.id } })}
            />
          </Card>
        </Animated.View>
      ))}

      {!!review?.suggestions.length && <Text style={section}>{tr('admin.suggestions')}</Text>}
      {review?.suggestions.map((s, i) => (
        <Animated.View key={`s${s.id}`} entering={motion.stagger(i)} exiting={motion.exit} layout={motion.layout}>
          <SuggestionCard
            text={s.text}
            rank={s.kind === 'rank'}
            meta={tr('admin.votes', { up: s.up, down: s.down, author: s.author })}
            lang={s.lang}
            busy={act.isPending}
            onReject={() => act.mutate({ path: `/suggestions/${s.id}/reject`, drop: { suggestion: s.id } })}
            onApprove={(tags) => act.mutate({ path: `/suggestions/${s.id}/approve`, json: tags.length ? { tags } : {}, drop: { suggestion: s.id } })}
          />
        </Animated.View>
      ))}

      <ConfirmModal
        visible={removing != null}
        title={tr('admin.removeTitle')}
        confirmLabel={tr('admin.remove')}
        busy={act.isPending}
        confirmDisabled={!reason.trim()}
        onCancel={() => setRemoving(null)}
        onConfirm={() =>
          removing != null && act.mutate({ path: `/packs/${removing}/remove`, json: { reason: reason.trim() }, drop: { pack: removing } })
        }>
        <TextInput
          accessibilityLabel={tr('admin.reason')}
          placeholder={tr('admin.reason')}
          placeholderTextColor={t.mut}
          value={reason}
          onChangeText={setReason}
          maxLength={200}
          autoFocus
          style={{
            height: 44,
            paddingHorizontal: 14,
            borderRadius: 14,
            backgroundColor: t.card2,
            fontFamily: t.font.regular,
            fontSize: 16,
            color: t.txt,
          }}
        />
      </ConfirmModal>
    </Screen>
  );
}

/** A queued suggestion: the question, its votes, the deck tones to file it under, reject / approve. */
function SuggestionCard({
  text,
  rank,
  meta,
  lang,
  busy,
  onReject,
  onApprove,
}: {
  text: string;
  rank: boolean;
  meta: string;
  lang: string;
  busy: boolean;
  onReject: () => void;
  onApprove: (tags: PackTag[]) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const [tags, setTags] = useState<PackTag[]>([]);
  return (
    <Card style={{ padding: 16, gap: 10 }}>
      <QuestionText text={text} rank={rank} style={{ fontFamily: t.font.semibold, fontSize: 16, lineHeight: 22, color: t.txt }} />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {meta}
        </Text>
        <MiniTag label={lang.toUpperCase()} />
      </View>
      <View style={{ flexDirection: 'row', gap: 6 }}>
        {DECK_TAGS.map((id) => {
          const on = tags.includes(id);
          const Icon = TAG_ICONS[id];
          return (
            <PressableScale
              key={id}
              accessibilityRole="button"
              accessibilityLabel={tr(`packs.tags.${id}`)}
              accessibilityState={{ selected: on }}
              onPress={() => setTags((l) => (on ? l.filter((x) => x !== id) : [...l, id]))}
              style={{
                width: 34,
                height: 34,
                borderRadius: 17,
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: on ? t.acc : t.card2,
              }}>
              <Icon size={16} color={on ? t.accTxt : t.mut} />
            </PressableScale>
          );
        })}
      </View>
      <Actions busy={busy} onReject={onReject} rejectLabel={tr('admin.reject')} onApprove={() => onApprove(tags)} />
    </Card>
  );
}

/** Reject (or remove) and approve, side by side. */
function Actions({ busy, onReject, rejectLabel, onApprove }: { busy: boolean; onReject: () => void; rejectLabel: string; onApprove: () => void }) {
  const t = useTheme();
  const tr = useT();
  const pill = {
    flex: 1,
    height: 42,
    borderRadius: t.radius.pill,
    flexDirection: 'row' as const,
    gap: 6,
    alignItems: 'center' as const,
    justifyContent: 'center' as const,
  };
  return (
    <View style={{ flexDirection: 'row', gap: 10, marginTop: 4 }}>
      <PressableScale accessibilityRole="button" disabled={busy} onPress={onReject} style={[pill, { backgroundColor: t.card2 }]}>
        <X size={16} color={t.tag} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>{rejectLabel}</Text>
      </PressableScale>
      <PressableScale accessibilityRole="button" disabled={busy} onPress={onApprove} style={[pill, { backgroundColor: t.acc }]}>
        <Check size={16} color={t.accTxt} />
        <Text style={{ fontFamily: t.font.bold, fontSize: 14, color: t.accTxt }}>{tr('admin.approve')}</Text>
      </PressableScale>
    </View>
  );
}
