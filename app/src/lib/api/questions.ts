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

export type NewQuestion =
  | { text: string; kind: 'choose'; items: string[] }
  | { text: string; kind: 'said' }
  | { text: string; kind: 'pick' | 'rank'; anonymous?: boolean };

export const addQuestion = (question: NewQuestion) => api('/questions', { method: 'POST', json: question });

export const deleteQuestion = (id: number) => api(`/questions/${id}`, { method: 'DELETE' });

export const deleteQuestions = (ids: number[]) =>
  api<{ deleted: number[]; skipped: { id: number; error: string }[] }>('/questions/delete', { method: 'POST', json: { ids } });

export const importQuestions = (lines: string[]) =>
  api<{ added: number; skipped: number }>('/questions/import', { method: 'POST', json: { text: lines.join('\n') } });

export const shuffleQueue = () => api<{ shuffled: number }>('/questions/shuffle', { method: 'POST' });

export const emptyQueue = () => api<{ deleted: number }>('/questions/queue', { method: 'DELETE' });

export const reorderQueue = (ids: number[]) => api('/questions/order', { method: 'POST', json: { ids } });

export const askForQuestions = () => api('/questions/ask', { method: 'POST' });

export type QueuedQuestion = Queue['custom'][number];
