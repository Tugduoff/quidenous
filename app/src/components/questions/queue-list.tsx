import type { ComponentRef, RefObject } from 'react';
import { Pressable, type ScrollView, View } from 'react-native';
import Animated, { LayoutAnimationConfig } from 'react-native-reanimated';
import { QueueRow } from '@/components/questions/queue-row';
import { type Selection, SelectableRow } from '@/components/ui/selection';
import { ReorderList, type ScrollTracker } from '@/components/vote/reorder-list';
import { useMembers } from '@/lib/api/group';
import type { QueuedQuestion } from '@/lib/api/questions';
import { useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { tidy } from '@/lib/utils/text';

/**
 * The upcoming questions. Sortable: hold one to drag it elsewhere; held and let go on someone else's, report it.
 * Selecting swaps it for the plain list (and back) at once: the old list's rows don't fade out.
 */
export function QueueList({
  queue,
  sortable,
  sel,
  selectable,
  isNew,
  scroller,
  tracker,
  onDragChange,
  onReorder,
  perQuestion,
  canDelete,
  othersQuestion,
  onReport,
  onDelete,
}: {
  queue: QueuedQuestion[];
  sortable: boolean;
  sel: Selection<number>;
  selectable: number[] | undefined;
  isNew: (id: number) => boolean;
  scroller: RefObject<ComponentRef<typeof ScrollView> | null>;
  tracker: ScrollTracker;
  onDragChange: (dragging: boolean) => void;
  onReorder: (ids: number[]) => void;
  perQuestion: boolean;
  canDelete: (q: QueuedQuestion) => boolean;
  othersQuestion: (q: QueuedQuestion) => boolean;
  onReport: (id: number) => void;
  onDelete: (q: QueuedQuestion) => void;
}) {
  const tr = useT();
  const { byId } = useMembers();
  const row = (q: QueuedQuestion) => (
    <QueueRow
      q={q}
      member={q.author_id != null ? byId.get(q.author_id) : undefined}
      perQuestion={perQuestion}
      canDelete={canDelete(q)}
      onDelete={() => onDelete(q)}
    />
  );
  const rowActions = (q: QueuedQuestion) => [
    ...(othersQuestion(q) ? [{ name: 'report', label: tr('moderation.reportQuestion') }] : []),
    ...(canDelete(q) ? [{ name: 'delete', label: tr('questions.deleteQuestion') }] : []),
  ];
  if (sortable && !sel.on)
    return (
      <LayoutAnimationConfig skipExiting>
        <View>
          <ReorderList
            items={queue}
            render={row}
            isNew={isNew}
            scroller={scroller}
            tracker={tracker}
            onDragChange={onDragChange}
            onReorder={onReorder}
            onStill={(id) => {
              const q = queue.find((x) => x.id === id);
              if (q && othersQuestion(q)) onReport(id);
            }}
          />
        </View>
      </LayoutAnimationConfig>
    );
  return (
    <LayoutAnimationConfig skipExiting>
      <View style={{ gap: 16 }}>
        {queue.map((q) => (
          <Animated.View key={q.id} entering={isNew(q.id) ? motion.enter : undefined} exiting={motion.exit} layout={motion.layout}>
            {sel.on ? (
              <SelectableRow sel={sel} id={q.id} label={tidy(q.text)} disabled={!selectable?.includes(q.id)}>
                {row(q)}
              </SelectableRow>
            ) : (
              <Pressable
                delayLongPress={400}
                onLongPress={othersQuestion(q) ? () => onReport(q.id) : undefined}
                accessibilityActions={rowActions(q)}
                onAccessibilityAction={(e) => {
                  if (e.nativeEvent.actionName === 'report') onReport(q.id);
                  if (e.nativeEvent.actionName === 'delete') onDelete(q);
                }}>
                {row(q)}
              </Pressable>
            )}
          </Animated.View>
        ))}
      </View>
    </LayoutAnimationConfig>
  );
}
