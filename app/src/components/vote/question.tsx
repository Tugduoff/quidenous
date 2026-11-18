import { type Round, retireRoundQuestion, roundKeys } from '@/lib/api/rounds';
import { useTheme } from '@/lib/theme';
import { useT, t as translate } from '@/lib/i18n';
import { useState } from 'react';
import { useMembers, useGroup } from '@/lib/api/group';
import { useQueryClient } from '@tanstack/react-query';
import { useFeatures } from '@/lib/api/features';
import { askConfirm } from '@/lib/confirm';
import { showToast } from '@/lib/toast';
import { authError } from '@/lib/api/errors';
import { historyKeys } from '@/lib/api/history';
import { confirmPublic, shareText } from '@/lib/device/share';
import { publicLink } from '@/lib/api/share';
import { resultMessage, questionMessage } from '@/lib/links';
import Animated from 'react-native-reanimated';
import { motion } from '@/lib/motion';
import { View, Pressable, Text } from 'react-native';
import { Tag } from '@/components/ui/tag';
import { AnonymousBadge } from '@/components/vote/anonymous';
import { ChallengeButton } from '@/components/community/challenge';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Check, Share2 } from 'lucide-react-native';
import { Display } from '@/components/ui/text';
import { tidy } from '@/lib/utils/text';
import { useFreshQuestion } from '@/lib/hooks/use-fresh-question';

export function Question({ round, result }: { round: Round; result?: boolean }) {
  const t = useTheme();
  const tr = useT();
  const fresh = useFreshQuestion(round.id, !!result);
  const { byId } = useMembers();
  const qc = useQueryClient();
  const { data: group } = useGroup();
  const { moderation } = useFeatures();
  // The owner can take an abusive question of the room out of its result (long-press on the question).
  const retire =
    result && round.custom && moderation && group?.my_role === 'owner'
      ? async () => {
          if (!(await askConfirm(translate('vote.retireAsk'), translate('vote.retireMessage'), translate('vote.retireAction')))) return;
          await retireRoundQuestion(round.id).catch((e) => showToast(authError(e)));
          qc.invalidateQueries({ queryKey: roundKeys.all });
          qc.invalidateQueries({ queryKey: historyKeys.all });
        }
      : undefined;
  const [copied, setCopied] = useState(false);
  const share = async () => {
    // The link opens a page whose preview is a picture of the card (the result's podium too, once out).
    if (!(await confirmPublic())) return;
    const link = await publicLink(round.id, !!result);
    const text = result ? resultMessage(round, byId) : questionMessage(round);
    const outcome = await shareText(link ? `${text.split('\n').slice(0, -1).join('\n')}\n${link}` : text);
    if (outcome === 'copied') {
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    }
  };
  return (
    <Animated.View entering={fresh ? motion.question : undefined} style={{ gap: 14 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, flexShrink: 1 }}>
          <Tag label={`${tr(kindLabel(round, result))} · #${round.number}`} tone={result ? 'acc' : 'tag'} />
          {round.anonymous && <AnonymousBadge />}
        </View>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          {result && <ChallengeButton round={round} />}
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr(copied ? 'vote.linkCopied' : result ? 'vote.shareResult' : 'vote.shareQuestion')}
            onPress={share}
            hitSlop={8}
            style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
            {copied ? <Check size={18} color={t.accInk} /> : <Share2 size={18} color={t.txt} />}
          </PressableScale>
        </View>
      </View>
      <View style={{ gap: 6 }}>
        <Pressable disabled={!retire} onLongPress={retire} delayLongPress={500} accessibilityHint={retire ? tr('vote.retireHint') : undefined}>
          <Display size={result ? 28 : 34}>{tidy(round.question)}</Display>
        </Pressable>
        {/* A deck question that came from someone's suggestion. */}
        {!!round.suggested_by && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
            {tr('community.credit', { name: round.suggested_by })}
          </Text>
        )}
      </View>
    </Animated.View>
  );
}

/** The tag over a question: its kind (a choose or ranking question says so even when it's ours), or "Résultat". */
const kindLabel = (round: Round, result?: boolean) =>
  result
    ? 'vote.kind.result'
    : round.kind === 'rank'
      ? 'vote.kind.rank'
      : round.kind === 'choose'
        ? 'vote.kind.choose'
        : round.kind === 'said'
          ? 'vote.kind.said'
          : round.custom
            ? 'vote.kind.custom'
            : 'vote.kind.pick';
