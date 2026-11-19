import { Shuffle, X } from 'lucide-react-native';
import type { Dispatch, SetStateAction } from 'react';
import { Pressable, Text, View } from 'react-native';
import { DraftLineRow } from '@/components/questions/draft-line-row';
import { Card } from '@/components/ui/card';
import { Toggle } from '@/components/ui/toggle';
import { useFeatures } from '@/lib/api/features';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import type { DraftLine } from '@/lib/utils/import-draft';
import { shuffled } from '@/lib/utils/shuffle';

export function ImportDraftCard({
  draft,
  setDraft,
  mix,
  setMix,
  busy,
  onImport,
}: {
  draft: DraftLine[];
  setDraft: Dispatch<SetStateAction<DraftLine[] | null>>;
  mix: boolean;
  setMix: (mix: boolean) => void;
  busy: boolean;
  onImport: (lines: string[]) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const { anonymousVotes } = useFeatures();
  const chosen = draft.filter((l) => l.selected).map((l) => l.line);
  const toggle = (i: number) => setDraft((d) => d && d.map((l, j) => (j === i && !l.problem ? { ...l, selected: !l.selected } : l)));
  const allOn = draft.every((l) => l.selected || l.problem);
  const toggleAll = () => setDraft((d) => d && d.map((l) => (l.problem ? l : { ...l, selected: !allOn })));
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      <View
        style={{
          flexDirection: 'row',
          alignItems: 'center',
          justifyContent: 'space-between',
        }}>
        <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>
          {tr('questions.toImport', { chosen: chosen.length, total: draft.length })}
        </Text>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={allOn ? tr('questions.uncheckAll') : tr('questions.checkAll')}
          onPress={toggleAll}
          hitSlop={8}>
          <Text
            style={{
              fontFamily: t.font.semibold,
              fontSize: 14,
              color: t.accInk,
            }}>
            {allOn ? tr('questions.none') : tr('questions.all')}
          </Text>
        </Pressable>
      </View>
      <Toggle icon={Shuffle} label={tr('questions.randomOrder')} value={mix} onChange={setMix} />
      {draft.map((l, i) => (
        <DraftLineRow key={i} line={l} anonymousVotes={anonymousVotes} onToggle={() => toggle(i)} />
      ))}
      <View style={{ flexDirection: 'row', gap: 10 }}>
        <Pressable
          accessibilityRole="button"
          onPress={() => setDraft(null)}
          style={{
            flex: 1,
            height: 48,
            borderRadius: t.radius.pill,
            alignItems: 'center',
            justifyContent: 'center',
            flexDirection: 'row',
            gap: 6,
            backgroundColor: t.card2,
          }}>
          <X size={18} color={t.txt} />
          <Text
            style={{
              fontFamily: t.font.semibold,
              fontSize: 15,
              color: t.txt,
            }}>
            {tr('common.cancel')}
          </Text>
        </Pressable>
        <Pressable
          accessibilityRole="button"
          disabled={!chosen.length || busy}
          onPress={() => onImport(mix ? shuffled(chosen) : chosen)}
          style={{
            flex: 2,
            height: 48,
            borderRadius: t.radius.pill,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: chosen.length ? t.acc : t.card2,
          }}>
          <Text
            style={{
              fontFamily: t.font.bold,
              fontSize: 15,
              color: chosen.length ? t.accTxt : t.mut,
            }}>
            {busy ? tr('questions.importing') : tr('questions.importCount', { count: chosen.length })}
          </Text>
        </Pressable>
      </View>
    </Card>
  );
}
