import { useMutation, useQueryClient } from '@tanstack/react-query';
import { router, useLocalSearchParams } from 'expo-router';
import { Check, Download, Flag, Heart, Pencil, Share2, Star, Trash2 } from 'lucide-react-native';
import { useState } from 'react';
import { Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { IconButton, MiniTag, PageHeader, Stat, StatusBadge } from '@/components/pack-card';
import { Screen } from '@/components/screen';
import { askConfirm, Card, Dialog, DialogActions, DialogTitle, Display, PressableScale, QuestionText } from '@/components/ui';
import { api, ApiError } from '@/lib/api';
import { useT } from '@/lib/i18n';
import { motion, usePop } from '@/lib/motion';
import {
  isPackTag,
  packError,
  packKeys,
  packLink,
  type PackDetail,
  REPORT_REASONS,
  type ReportReason,
  TAG_ICONS,
  useCommunityMe,
  usePackDetail,
} from '@/lib/packs';
import { keys, useGroup, useRooms } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { shareText } from '@/lib/share';
import { useTheme } from '@/lib/theme';

/** One pack: its questions, like, install (into a room I pick when I have several), share, report; edit and delete (author); feature (admin). */
export default function PackScreen() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const id = Number(useLocalSearchParams<{ id: string }>().id);
  const { data: pack, isError } = usePackDetail(id);
  const { data: group } = useGroup();
  const { data: rooms } = useRooms();
  const { data: me } = useCommunityMe();
  const { room } = useSession();
  const here = room ?? group?.id ?? null;
  const several = (rooms?.length ?? 0) > 1;
  const [note, setNote] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);
  const [reporting, setReporting] = useState(false);
  // Rooms it went into from this page (the server only tells about the current room).
  const [installedIn, setInstalledIn] = useState<number[]>([]);
  const isIn = (roomId: number | null) => roomId != null && (installedIn.includes(roomId) || (roomId === here && !!pack?.installed));

  const patch = (p: Partial<PackDetail>) => qc.setQueryData<PackDetail>(packKeys.detail(id), (old) => old && { ...old, ...p });
  const refreshLists = () => qc.invalidateQueries({ queryKey: packKeys.all, predicate: (q) => q.queryKey[1] !== 'detail' });

  const install = useMutation({
    mutationFn: (roomId: number) => api<{ added: number; skipped: number }>(`/packs/${id}/install`, { method: 'POST', room: roomId }),
    onSuccess: (r, roomId) => {
      const name = rooms?.find((x) => x.id === roomId)?.name ?? group?.name ?? '';
      setNote(
        r.skipped
          ? tr('packs.installedSkipped', { added: r.added, room: name, skipped: r.skipped })
          : tr('packs.installed', { added: r.added, room: name }),
      );
      setInstalledIn((l) => [...l, roomId]);
      if (roomId === here) {
        patch({ installed: true });
        qc.invalidateQueries({ queryKey: keys.queue });
        qc.invalidateQueries({ queryKey: keys.current });
      }
      // Its install count (once per room) comes back from the server.
      qc.invalidateQueries({ queryKey: packKeys.all });
    },
    onError: (e) => setNote(tr(e instanceof ApiError ? packError(e.code) : 'common.failed')),
  });
  const like = useMutation({
    mutationFn: (on: boolean) => api<{ likes: number; liked: boolean }>(`/packs/${id}/like`, { method: on ? 'POST' : 'DELETE' }),
    onMutate: (on) => pack && patch({ liked: on, likes: Math.max(0, pack.likes + (on ? 1 : -1)) }),
    onSuccess: (r) => patch(r),
    onError: () => qc.invalidateQueries({ queryKey: packKeys.detail(id) }),
    onSettled: refreshLists,
  });
  const feature = useMutation({
    mutationFn: (featured: boolean) => api<{ featured: boolean }>(`/admin/packs/${id}/feature`, { method: 'POST', json: { featured } }),
    onSuccess: (r) => {
      patch(r);
      refreshLists();
    },
    onError: () => setNote(tr('common.failed')),
  });
  const remove = useMutation({
    mutationFn: () => api(`/packs/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: packKeys.all });
      router.back();
    },
    onError: () => setNote(tr('common.failed')),
  });
  const pop = usePop(pack?.liked, 1.25, !!pack?.liked);

  if (!pack)
    return (
      <Screen tabs={false}>
        <PageHeader fallback="/packs" />
        {isError && <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>{tr('packs.none')}</Text>}
      </Screen>
    );

  const listed = pack.status === 'public';
  const installable = listed || (pack.mine && pack.status === 'review');
  const installedHere = isIn(here);
  const share = async () => {
    const r = await shareText(tr('packs.shareMessage', { name: pack.name, link: packLink(id) }));
    if (r === 'copied') setNote(tr('packs.linkCopied'));
  };
  const askRemove = async () => {
    if (await askConfirm(tr('packs.remove'), undefined, tr('packs.removeAction'))) remove.mutate();
  };
  const tags = pack.tags.filter(isPackTag);
  const otherLang = !!pack.language && !!group?.language && pack.language !== group.language;
  const marks = pack.featured || pack.spicy || otherLang || tags.length > 0 || (pack.mine && !listed);

  return (
    <Screen
      tabs={false}
      footer={
        installable && (
          <PressableScale
            accessibilityRole="button"
            disabled={install.isPending || here == null}
            onPress={() => (several ? setPicking(true) : here != null && install.mutate(here))}
            style={{
              height: 52,
              paddingHorizontal: 20,
              borderRadius: t.radius.pill,
              flexDirection: 'row',
              gap: 8,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: installedHere && !several ? t.card2 : t.acc,
            }}>
            {installedHere && !several ? <Check size={18} color={t.accInk} /> : <Download size={18} color={t.accTxt} />}
            <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 16, color: installedHere && !several ? t.accInk : t.accTxt }}>
              {several ? tr('packs.installIn') : installedHere ? tr('packs.installedHere') : tr('packs.install')}
            </Text>
          </PressableScale>
        )
      }>
      <PageHeader fallback="/packs">
        {me?.admin && listed && (
          <IconButton
            icon={Star}
            label={tr(pack.featured ? 'packs.unfeature' : 'packs.feature')}
            on={pack.featured}
            disabled={feature.isPending}
            onPress={() => feature.mutate(!pack.featured)}
          />
        )}
        {listed && <IconButton icon={Share2} label={tr('packs.share')} onPress={share} />}
        {pack.mine && pack.status !== 'removed' && (
          <IconButton icon={Pencil} label={tr('packs.edit')} onPress={() => router.push({ pathname: '/pack/edit', params: { id: String(id) } })} />
        )}
        {pack.mine ? (
          <IconButton icon={Trash2} label={tr('packs.removeAction')} disabled={remove.isPending} onPress={askRemove} />
        ) : (
          <IconButton icon={Flag} label={tr('packs.report')} onPress={() => setReporting(true)} />
        )}
      </PageHeader>

      <View style={{ gap: 6 }}>
        <Display size={28}>{pack.name}</Display>
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
          {`${tr('packs.questions', { count: pack.questions.length })} · `}
          <Text
            accessibilityRole="link"
            onPress={() => router.push(`/creator/${pack.author_id}`)}
            style={{ fontFamily: t.font.semibold, color: t.accInk }}>
            {pack.author}
          </Text>
        </Text>
        {marks && (
          <View style={{ flexDirection: 'row', flexWrap: 'wrap', alignItems: 'center', gap: 6 }}>
            {pack.mine && <StatusBadge status={pack.status} />}
            {pack.featured && (
              <View
                style={{
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 4,
                  height: 22,
                  paddingHorizontal: 8,
                  borderRadius: 11,
                  backgroundColor: t.accSoft,
                }}>
                <Star size={12} color={t.accInk} fill={t.accInk} />
                <Text style={{ fontFamily: t.font.bold, fontSize: 11, color: t.accInk }}>{tr('packs.featured')}</Text>
              </View>
            )}
            {pack.spicy && <MiniTag label={tr('packs.spicy')} tone="hot" />}
            {otherLang && <MiniTag label={pack.language!.toUpperCase()} />}
            {tags.map((tag) => {
              const Icon = TAG_ICONS[tag];
              return (
                <View
                  key={tag}
                  style={{
                    flexDirection: 'row',
                    alignItems: 'center',
                    gap: 4,
                    height: 22,
                    paddingHorizontal: 8,
                    borderRadius: 11,
                    backgroundColor: t.card2,
                  }}>
                  <Icon size={12} color={t.mut} />
                  <Text style={{ fontFamily: t.font.semibold, fontSize: 11, color: t.mut }}>{tr(`packs.tags.${tag}`)}</Text>
                </View>
              );
            })}
          </View>
        )}
        {pack.mine && pack.status === 'removed' && !!pack.removed_reason && (
          <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag }}>
            {tr('packs.removedReason', { reason: pack.removed_reason })}
          </Text>
        )}
        {!!pack.description && <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.txt }}>{pack.description}</Text>}
      </View>

      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 14 }}>
        {pack.mine ? (
          <Stat icon={Heart} value={pack.likes} label={tr('packs.likes', { count: pack.likes })} />
        ) : (
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('packs.like')}
            accessibilityState={{ selected: pack.liked }}
            onPress={() => like.mutate(!pack.liked)}
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

      {note && (
        <Animated.Text entering={motion.enter} style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>
          {note}
        </Animated.Text>
      )}

      <Card style={{ padding: 16, gap: 10 }}>
        {pack.questions.map((q, i) => (
          <QuestionText
            key={i}
            text={q.text}
            rank={q.kind === 'rank'}
            style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 20, color: t.txt }}
          />
        ))}
      </Card>

      <Dialog visible={picking} onClose={() => setPicking(false)}>
        <DialogTitle title={tr('packs.installIn')} />
        <View style={{ gap: 8 }}>
          {rooms?.map((r) => {
            const done = isIn(r.id);
            return (
              <PressableScale
                key={r.id}
                accessibilityRole="button"
                disabled={install.isPending}
                onPress={() => {
                  setPicking(false);
                  install.mutate(r.id);
                }}
                style={{
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 10,
                  height: 48,
                  paddingHorizontal: 14,
                  borderRadius: 14,
                  backgroundColor: t.card2,
                }}>
                <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                  {r.name}
                </Text>
                {done && <Check size={18} color={t.accInk} accessibilityLabel={tr('packs.installedHere')} />}
              </PressableScale>
            );
          })}
        </View>
      </Dialog>

      <ReportDialog
        visible={reporting}
        onClose={() => setReporting(false)}
        onSent={() => {
          setReporting(false);
          setNote(tr('packs.reported'));
        }}
        id={id}
      />
    </Screen>
  );
}

/** Report a pack: a reason, and a few words for "other". */
function ReportDialog({ visible, onClose, onSent, id }: { visible: boolean; onClose: () => void; onSent: () => void; id: number }) {
  const t = useTheme();
  const tr = useT();
  const [reason, setReason] = useState<ReportReason | null>(null);
  const [details, setDetails] = useState('');
  const send = useMutation({
    mutationFn: () => api(`/packs/${id}/report`, { method: 'POST', json: { reason, details: details.trim() || undefined } }),
    onSuccess: onSent,
    // Already reported by me: same answer.
    onError: (e) => (e instanceof ApiError && e.status === 409 ? onSent() : undefined),
  });
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title={tr('packs.reportTitle')} />
      <View accessibilityRole="radiogroup" style={{ gap: 8 }}>
        {REPORT_REASONS.map((r) => {
          const on = r === reason;
          return (
            <PressableScale
              key={r}
              accessibilityRole="radio"
              accessibilityState={{ selected: on }}
              onPress={() => setReason(r)}
              style={{
                flexDirection: 'row',
                alignItems: 'center',
                height: 46,
                paddingHorizontal: 14,
                borderRadius: 14,
                borderWidth: 2,
                borderColor: on ? t.accInk : 'transparent',
                backgroundColor: on ? t.accSoft : t.card2,
              }}>
              <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr(`packs.reasons.${r}`)}</Text>
              {on && <Check size={18} color={t.accInk} />}
            </PressableScale>
          );
        })}
      </View>
      {reason === 'other' && (
        <TextInput
          accessibilityLabel={tr('packs.reportDetails')}
          placeholder={tr('packs.reportDetails')}
          placeholderTextColor={t.mut}
          value={details}
          onChangeText={setDetails}
          maxLength={300}
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
      )}
      {send.isError && !(send.error instanceof ApiError && send.error.status === 409) && (
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag }}>{tr('common.failed')}</Text>
      )}
      <DialogActions confirmLabel={tr('packs.report')} onConfirm={() => send.mutate()} onCancel={onClose} busy={send.isPending} disabled={!reason} />
    </Dialog>
  );
}
