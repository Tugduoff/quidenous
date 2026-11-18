import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus } from 'lucide-react-native';
import { type ComponentRef, type RefObject, useState } from 'react';
import { TextInput, View } from 'react-native';
import { ChooseItemsEditor } from '@/components/questions/choose-items-editor';
import { AnonymousToggle } from '@/components/vote/anonymous';
import { Card } from '@/components/ui/card';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Segmented } from '@/components/ui/segmented';
import { sendOnEnter } from '@/components/ui/send-on-enter';
import { ApiError } from '@/lib/api/client';
import { useFeatures } from '@/lib/api/features';
import { addQuestion, questionKeys } from '@/lib/api/questions';
import { type QuestionKind, roundKeys } from '@/lib/api/rounds';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { itemsOk, splitTag } from '@/lib/utils/import-draft';

const MIN = 8;

export function QuestionComposer({ inputRef, perQuestion }: { inputRef: RefObject<ComponentRef<typeof TextInput> | null>; perQuestion: boolean }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const features = useFeatures();
  const [text, setText] = useState('');
  const [rank, setRank] = useState(false);
  const [choose, setChoose] = useState(false);
  const [items, setItems] = useState<string[]>(['', '']);
  const [said, setSaid] = useState(false);
  const [anon, setAnon] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);
  const typed = splitTag(text.trim());
  const typedChoose = typed.kind === 'choose' && features.choose;
  const kind: QuestionKind = said && features.said ? 'said' : choose || typedChoose ? 'choose' : rank || typed.rank ? 'rank' : 'pick';
  const chosenItems = choose ? items.map((i) => i.trim()) : (typed.items ?? []);
  const anonymous = perQuestion && kind !== 'choose' && kind !== 'said' && (anon || typed.anonymous);
  const sameItems = new Set(chosenItems.filter(Boolean).map((i) => i.toLowerCase())).size < chosenItems.filter(Boolean).length;
  const add = useMutation({
    meta: { quiet: (e) => e instanceof ApiError && e.code === 'bad_items' },
    mutationFn: () =>
      addQuestion(
        kind === 'choose'
          ? { text: typed.text, kind, items: chosenItems }
          : kind === 'said'
            ? { text: typed.text, kind }
            : { text: typed.text, kind, ...(anonymous ? { anonymous } : {}) },
      ),
    onSuccess: () => {
      setText('');
      setRank(false);
      setChoose(false);
      setSaid(false);
      setItems(['', '']);
      setAnon(false);
      setAddError(null);
      qc.invalidateQueries({ queryKey: questionKeys.queue });
      qc.invalidateQueries({ queryKey: roundKeys.current });
    },
    onError: (e) => setAddError(e instanceof ApiError && e.code === 'bad_items' ? tr('questions.badItems') : null),
  });
  const ok = typed.text.length >= MIN && (kind !== 'choose' || itemsOk(chosenItems)) && (typed.kind !== 'choose' || typedChoose) && !add.isPending;
  return (
    <Card style={{ paddingBottom: 10 }}>
      <View style={{ flexDirection: 'row', alignItems: 'flex-end', paddingRight: 10 }}>
        <TextInput
          ref={inputRef}
          accessibilityLabel={tr('questions.newQuestion')}
          placeholder={
            kind === 'said'
              ? tr('said.composer.placeholder')
              : choose
                ? tr('questions.placeholderChoose')
                : rank
                  ? tr('questions.placeholderRank')
                  : tr('questions.placeholder')
          }
          placeholderTextColor={t.mut}
          value={text}
          onChangeText={setText}
          multiline
          maxLength={200}
          {...sendOnEnter(() => ok && add.mutate())}
          style={{
            flex: 1,
            minWidth: 0,
            minHeight: 76,
            padding: 16,
            paddingBottom: 6,
            fontFamily: t.font.regular,
            fontSize: 16,
            color: t.txt,
            textAlignVertical: 'top',
          }}
        />
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr('common.add')}
          disabled={!ok}
          onPress={() => add.mutate()}
          style={{
            width: 44,
            height: 44,
            borderRadius: 22,
            backgroundColor: ok ? t.acc : t.card2,
            alignItems: 'center',
            justifyContent: 'center',
            marginBottom: 6,
          }}>
          <Plus size={24} color={ok ? t.accTxt : t.mut} />
        </PressableScale>
      </View>
      {choose && <ChooseItemsEditor items={items} setItems={setItems} onSubmit={() => ok && add.mutate()} sameItems={sameItems} error={addError} />}
      <View style={{ paddingHorizontal: 10, flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <View style={{ flex: 1 }}>
          <Segmented
            fill
            label={tr('questions.kind')}
            value={rank ? 'rank' : choose ? 'choose' : kind === 'said' ? 'said' : 'pick'}
            onChange={(k) => {
              setRank(k === 'rank');
              setChoose(k === 'choose');
              setSaid(k === 'said');
              setAddError(null);
            }}
            options={[
              { id: 'pick', name: tr('vote.kind.pick') },
              { id: 'rank', name: tr('vote.kind.rank') },
              ...(features.choose ? [{ id: 'choose' as const, name: tr('vote.kind.choose') }] : []),
              ...(features.said ? [{ id: 'said' as const, name: tr('said.short') }] : []),
            ]}
          />
        </View>
        {perQuestion && kind !== 'choose' && kind !== 'said' && <AnonymousToggle value={anonymous} onChange={setAnon} />}
      </View>
    </Card>
  );
}
