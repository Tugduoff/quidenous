import { type ComponentRef, useRef, useState } from 'react';
import { Platform, type ScrollView, type TextInput } from 'react-native';
import { ModerationDialog, type ModTarget } from '@/components/moderation/moderation';
import { DeleteQuestionModal } from '@/components/questions/delete-question-modal';
import { ImportDraftCard } from '@/components/questions/import-draft-card';
import { LockedNotice } from '@/components/questions/locked-notice';
import { QuestionComposer } from '@/components/questions/question-composer';
import { QuestionSources } from '@/components/questions/question-sources';
import { QueueHeader } from '@/components/questions/queue-header';
import { QueueList } from '@/components/questions/queue-list';
import { QueueSelectionBar } from '@/components/questions/queue-selection-bar';
import { useScrollTracker } from '@/components/vote/reorder-list';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { useSelection } from '@/components/ui/selection';
import { Display } from '@/components/ui/text';
import { Tip } from '@/components/ui/tip';
import { useFeatures } from '@/lib/api/features';
import { useGroup } from '@/lib/api/group';
import { type QueuedQuestion, useQueue } from '@/lib/api/questions';
import { useQuestionHandoff } from '@/lib/hooks/use-question-handoff';
import { useQuestionImport } from '@/lib/hooks/use-question-import';
import { useQueueReorder } from '@/lib/hooks/use-queue-reorder';
import { useT } from '@/lib/i18n';
import { useArrivals } from '@/lib/motion';
import { useSession } from '@/lib/session';

export default function QuestionsScreen() {
  const tr = useT();
  const queue = useQueue();
  const { data } = queue;
  const isNew = useArrivals(data?.custom.map((q) => q.id));
  const { data: group } = useGroup();
  const { me } = useSession();
  const features = useFeatures();
  const [status, setStatus] = useState<string | null>(null);
  const [toDelete, setToDelete] = useState<{ id: number; text: string } | null>(null);
  const [toReport, setToReport] = useState<ModTarget | null>(null);
  const perQuestion = features.anonymousVotes && group?.settings.anonymous_votes === 'per_question';
  const importing = useQuestionImport(setStatus);
  const composer = useRef<ComponentRef<typeof TextInput>>(null);
  useQuestionHandoff(importing.review, () => composer.current?.focus());
  const scroller = useRef<ComponentRef<typeof ScrollView>>(null);
  const tracker = useScrollTracker();
  const [dragging, setDragging] = useState(false);
  const reordered = useQueueReorder(setStatus);
  const locked = group?.settings.questions_by === 'owner' && group.my_role !== 'owner';
  const sortable = features.reorder && !locked && (data?.custom.length ?? 0) > 1;
  const othersQuestion = (q: QueuedQuestion) => q.author_id != null && q.author_id !== me?.id;
  const canDelete = (q: QueuedQuestion) => q.author_id === me?.id || group?.my_role === 'owner';
  const canMove = features.reorder && !locked;
  const canDeleteMany = features.batchDelete && !locked;
  const canSelect = canMove || canDeleteMany;
  const selectable = data?.custom.filter((q) => canMove || canDelete(q)).map((q) => q.id);
  const sel = useSelection(selectable, canSelect);
  return (
    <Screen
      scroller={scroller}
      onScroll={tracker.onScroll}
      onContentSizeChange={tracker.onContentSizeChange}
      scrollEnabled={Platform.OS === 'web' || !dragging}
      footer={
        sel.on ? (
          <QueueSelectionBar
            sel={sel}
            queue={data?.custom}
            canMove={canMove}
            canDeleteMany={canDeleteMany}
            canDelete={canDelete}
            onReorder={reordered}
            onStatus={setStatus}
          />
        ) : undefined
      }>
      <Display size={30}>{tr('questions.title')}</Display>
      {locked ? <LockedNotice community={features.community} /> : null}
      {locked || sel.on ? null : (
        <>
          <Tip id="questions" text={tr('questions.tip')} />
          <QuestionComposer inputRef={composer} perQuestion={perQuestion} />
          <QuestionSources importing={importing.send.isPending} onPickFile={importing.pick} />
        </>
      )}
      {importing.draft && !sel.on && (
        <ImportDraftCard
          draft={importing.draft}
          setDraft={importing.setDraft}
          mix={importing.mix}
          setMix={importing.setMix}
          busy={importing.send.isPending}
          onImport={importing.send.mutate}
        />
      )}
      <QueueHeader
        count={data?.custom.length ?? 0}
        locked={locked}
        status={status}
        onStatus={setStatus}
        sel={sel}
        canSelect={canSelect && !!selectable?.length}
      />
      <QueryState query={queue} />
      {data ? (
        <QueueList
          queue={data.custom}
          sortable={sortable}
          sel={sel}
          selectable={selectable}
          isNew={isNew}
          scroller={scroller}
          tracker={tracker}
          onDragChange={setDragging}
          onReorder={reordered}
          perQuestion={perQuestion}
          canDelete={canDelete}
          othersQuestion={othersQuestion}
          onReport={(id) => setToReport({ kind: 'question', id })}
          onDelete={setToDelete}
        />
      ) : null}
      <ModerationDialog target={toReport} onClose={() => setToReport(null)} />
      <DeleteQuestionModal question={toDelete} onClose={() => setToDelete(null)} />
    </Screen>
  );
}
