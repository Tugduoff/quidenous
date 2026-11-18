import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api/client';
import { t } from '@/lib/i18n';
import type { Mood, Who } from '@/lib/utils/mascot';
import { shareCard } from '@/lib/device/share';
import { useGroup } from '@/lib/api/group';
import { useFeatures } from '@/lib/api/features';

export type WrappedScope = 'me' | 'room';
type WrappedPerson = { name: string; color: string; photo_url: string | null; avatar: string | null };
export type WrappedCard = {
  key: string;
  icon: string;
  mascot: { who: Who; mood: Mood } | null;
  title: string;
  big: string;
  line: string;
  people: WrappedPerson[];
  question?: string;
  shareable: boolean;
};
export type Wrapped = {
  year: number;
  scope: WrappedScope;
  available: boolean;
  preview: boolean;
  can_preview: boolean;
  from: string;
  until: string;
  room: string | null;
  cards: WrappedCard[];
};

const tz = () => -new Date().getTimezoneOffset();

const wrappedKeys = {
  all: ['wrapped'] as const,
  view: (scope: WrappedScope, preview: boolean) => ['wrapped', scope, preview ? 'preview' : 'live'] as const,
};

function normalize(w: Wrapped): Wrapped {
  const cards = Array.isArray(w?.cards) ? w.cards.filter((c) => c && typeof c.key === 'string') : [];
  return { ...w, cards: cards.map((c) => ({ ...c, people: Array.isArray(c.people) ? c.people : [], mascot: c.mascot ?? null })) };
}

export function useRoomWrappedOk() {
  const { data: group } = useGroup();
  return !!group && group.kind !== 'party';
}

export function useWrapped(scope: WrappedScope, preview = false) {
  const { wrapped } = useFeatures();
  const roomOk = useRoomWrappedOk();
  return useQuery({
    queryKey: wrappedKeys.view(scope, preview),
    queryFn: async () =>
      normalize(await api<Wrapped>(`/wrapped?scope=${scope}&tz=${tz()}${preview ? '&preview=1' : ''}`)),
    enabled: wrapped && (scope === 'me' || roomOk),
    staleTime: 10 * 60_000,
  });
}

export function useWrappedEntry(): { year: number; preview: boolean; ask: boolean } | null {
  const { data } = useWrapped('me');
  if (!data) return null;
  if (data.available) return { year: data.year, preview: data.preview, ask: false };
  if (data.can_preview) return { year: data.year, preview: true, ask: true };
  return null;
}

export function shareWrappedCard(w: Wrapped, card: WrappedCard) {
  return shareCard(
    () =>
      api<{ token: string; path: string }>('/wrapped/share', {
        method: 'POST',
        json: { scope: w.scope, year: w.year, key: card.key, tz: tz(), ...(w.preview && { preview: true }) },
      }),
    t('wrapped.shareText', { title: card.title, big: card.big, line: card.line }),
  );
}
