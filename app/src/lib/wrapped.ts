import { useQuery } from '@tanstack/react-query';

import { api } from './api';
import { useFeatures } from './auth';
import { t } from './i18n';
import type { Mood, Who } from './mascot';
import { useGroup } from './queries';
import { shareCard } from './share';

export type WrappedScope = 'me' | 'room';
/** Someone shown on a card (only people who share a room with me). */
export type WrappedPerson = { name: string; color: string; photo_url: string | null; avatar: string | null };
export type WrappedCard = {
  /** Unique in the list: 'intro', 'crush', 'nemesis', … */
  key: string;
  /** Lucide name, kebab-case. */
  icon: string;
  mascot: { who: Who; mood: Mood } | null;
  /** The small caps line, e.g. "Ton crush de l'année". */
  title: string;
  /** The big figure or name. */
  big: string;
  /** One punchline. */
  line: string;
  people: WrappedPerson[];
  /** Question cards: the question. */
  question?: string;
  /** Intro and outro can't be shared. */
  shareable: boolean;
};
/** `GET /wrapped`: the year's cards (texts in my language), or why there are none. */
export type Wrapped = {
  year: number;
  scope: WrappedScope;
  /** False outside the window (no cards) unless previewing. */
  available: boolean;
  /** Shown thanks to a preview (admin, dev server). */
  preview: boolean;
  /** I may ask for a preview. */
  can_preview: boolean;
  /** The window, `YYYY-MM-DD`. */
  from: string;
  until: string;
  /** scope=room: the room's name. */
  room: string | null;
  /** Empty when there isn't enough to tell. */
  cards: WrappedCard[];
};

/** Minutes east of UTC: the year's days are cut in my time zone. */
const tz = () => -new Date().getTimezoneOffset();

const wrappedKey = (scope: WrappedScope, preview: boolean) => ['wrapped', scope, preview ? 'preview' : 'live'] as const;

/** Whatever the server sent, with the lists the screens rely on. */
function normalize(w: Wrapped): Wrapped {
  const cards = Array.isArray(w?.cards) ? w.cards.filter((c) => c && typeof c.key === 'string') : [];
  return { ...w, cards: cards.map((c) => ({ ...c, people: Array.isArray(c.people) ? c.people : [], mascot: c.mascot ?? null })) };
}

/** A room's Wrapped exists for rooms only (not parties, not without a room). */
export function useRoomWrappedOk() {
  const { data: group } = useGroup();
  return !!group && group.kind !== 'party';
}

/** My year (`me`, across my rooms) or the room's, from a server that has Wrapped. `preview`: an admin looking early. */
export function useWrapped(scope: WrappedScope, preview = false) {
  const { wrapped } = useFeatures();
  const roomOk = useRoomWrappedOk();
  return useQuery({
    queryKey: wrappedKey(scope, preview),
    queryFn: async () =>
      normalize(await api<Wrapped>(`/wrapped?scope=${scope}&tz=${tz()}${preview ? '&preview=1' : ''}`)),
    enabled: wrapped && (scope === 'me' || roomOk),
    // The year's numbers barely move while I look; the screen and the entry points share one answer.
    staleTime: 10 * 60_000,
  });
}

/**
 * Whether to offer my Wrapped (Stats banner, profile row): during the window, or anytime as a preview for those who may.
 * `preview`: labelled "Aperçu"; `ask`: the screen must ask for the preview (out of season). Null: nothing to offer
 * (an older server, out of season).
 */
export function useWrappedEntry(): { year: number; preview: boolean; ask: boolean } | null {
  const { data } = useWrapped('me');
  if (!data) return null;
  if (data.available) return { year: data.year, preview: data.preview, ask: false };
  if (data.can_preview) return { year: data.year, preview: true, ask: true };
  return null;
}

/** Share one card: the server freezes it into a public link (its picture is the card), then the usual share flow. */
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
