import type { QuestionKind } from '@/lib/api/rounds';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';

export const questionKeys = {
  queue: ['queue'] as const,
};

export type Queue = {
  custom: { id: number; text: string; author_id: number | null; kind: QuestionKind; items?: string[] | null; anonymous?: boolean }[];
  deck_left: number;
};

export const useQueue = () => useQuery({ queryKey: questionKeys.queue, queryFn: () => api<Queue>('/questions/queue') });
