import { useState } from 'react';
import { Text, View } from 'react-native';
import { MiniTag } from '@/components/packs/pack-card';
import { useT } from '@/lib/i18n';
import type { PackTag } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';
import { QuestionText } from '@/components/ui/text';
import { TAG_ICONS } from '@/components/packs/tag-icons';
import { ReviewActions } from '@/components/admin/review-actions';

/** The built-in deck's tones a suggestion can join (none picked: the server files it under `potes`). */
const DECK_TAGS: PackTag[] = ['potes', 'pimente', 'boulot', 'famille'];

/** A queued suggestion: the question, its votes, the deck tones to file it under, reject / approve. */
export function SuggestionCard({
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
      <ReviewActions busy={busy} onReject={onReject} rejectLabel={tr('admin.reject')} onApprove={() => onApprove(tags)} />
    </Card>
  );
}
