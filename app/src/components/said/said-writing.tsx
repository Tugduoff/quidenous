import { useMutation, useQueryClient } from '@tanstack/react-query';
import { type ReactNode, useState } from 'react';
import { Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { AnswerButtons } from '@/components/said/answer-buttons';
import { AnswerField } from '@/components/said/answer-field';
import { MyAnswer } from '@/components/said/my-answer';
import { AvatarStack } from '@/components/ui/avatar';
import { Screen } from '@/components/ui/screen';
import { ApiError } from '@/lib/api/client';
import type { Member } from '@/lib/api/group';
import { answerCurrent, type Round, roundKeys, type SaidView } from '@/lib/api/rounds';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useTheme } from '@/lib/theme';

export function SaidWriting({
  round,
  said,
  byId,
  top,
  waitList,
}: {
  round: Round;
  said: SaidView;
  byId: Map<number, Member>;
  top: ReactNode;
  waitList: (done: number[], title: string) => ReactNode;
}) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const mine = said.my_answer;
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(mine?.text ?? '');
  const [error, setError] = useState<string | null>(null);
  const send = useMutation({
    meta: { quiet: true },
    mutationFn: (answer: string) => answerCurrent(answer),
    onSuccess: (r) => {
      setEditing(false);
      setError(null);
      if (r && typeof r === 'object' && 'id' in r) qc.setQueryData(roundKeys.current, r);
    },
    onError: (e) => {
      const late = e instanceof ApiError && (e.code === 'not_writing' || e.code === 'not_said' || e.code === 'no_open_round');
      setError(tr(late ? 'said.write.tooLate' : 'said.write.failed'));
    },
    onSettled: () => qc.invalidateQueries({ queryKey: roundKeys.current }),
  });
  const ok = text.trim().length > 0 && !send.isPending && text.trim() !== mine?.text;
  const submit = () => ok && send.mutate(text.trim());
  const writing = !mine || editing;
  const done = said.writer_ids.length;
  return (
    <Screen
      footer={
        writing && (
          <AnswerButtons
            editing={editing}
            ok={ok}
            busy={send.isPending}
            onSubmit={submit}
            onCancel={() => {
              setEditing(false);
              setError(null);
              setText(mine?.text ?? '');
            }}
          />
        )
      }>
      {top}
      {writing ? (
        <>
          {done > 0 && (
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
              <AvatarStack members={said.writer_ids.map((id) => byId.get(id))} max={6} />
              <Text style={{ fontFamily: t.font.regular, color: t.mut }}>
                {tr('said.write.written', { count: done, done, total: round.member_count })}
              </Text>
            </View>
          )}
          <AnswerField value={text} onChange={setText} onSubmit={submit} autoFocus={editing} error={error} />
        </>
      ) : (
        <>
          <Animated.View entering={motion.rise}>
            <MyAnswer
              text={mine.text}
              onEdit={() => {
                setText(mine.text);
                setEditing(true);
              }}
            />
          </Animated.View>
          <Text style={{ marginTop: -4, paddingHorizontal: 4, fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
            {tr('said.write.next')}
          </Text>
          {waitList(said.writer_ids, tr('said.write.written', { count: done, done, total: round.member_count }))}
        </>
      )}
    </Screen>
  );
}
