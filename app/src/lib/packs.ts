import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { Beer, BookOpen, Briefcase, Flame, House, type LucideIcon, PartyPopper, Sparkles } from 'lucide-react-native';

import { api, API_URL, type Member } from './api';
import type { TKey } from './i18n';

/** Pack categories (docs/COMMUNITY.md "Categories"): fixed ids, the app names and draws them. `pimente` means 16+. */
export const PACK_TAGS = ['potes', 'pimente', 'boulot', 'famille', 'soiree', 'culture', 'absurde'] as const;
export type PackTag = (typeof PACK_TAGS)[number];
export const TAG_ICONS: Record<PackTag, LucideIcon> = {
  potes: Beer,
  pimente: Flame,
  boulot: Briefcase,
  famille: House,
  soiree: PartyPopper,
  culture: BookOpen,
  absurde: Sparkles,
};
export const isPackTag = (id: string): id is PackTag => (PACK_TAGS as readonly string[]).includes(id);
export const MAX_TAGS = 3;

export type PackStatus = 'public' | 'review' | 'removed' | 'deleted';
export type PackKind = 'pick' | 'rank';
export type PackQuestion = { text: string; kind: PackKind };

/** A pack as lists show it. `installed`: in the current room. `status`/`removed_reason` only matter to its author (and admins). */
export type PackItem = {
  id: number;
  name: string;
  description: string;
  author: string;
  author_id: number;
  installs: number;
  likes: number;
  liked: boolean;
  count: number;
  mine: boolean;
  sample: string[];
  language?: 'fr' | 'en';
  tags: string[];
  spicy: boolean;
  featured: boolean;
  installed: boolean;
  status: PackStatus;
  version: number;
  removed_reason: string | null;
};
export type PackDetail = PackItem & { questions: PackQuestion[] };
export type PackSort = 'trending' | 'new' | 'liked' | 'featured';

export type Creator = Member & { installs: number; likes: number; packs: PackItem[] };
export type CommunityMe = { admin: boolean; show_spicy: boolean; suggestions_left: number };

export type ReportReason = 'offensive' | 'sexual' | 'spam' | 'other';
export const REPORT_REASONS: ReportReason[] = ['offensive', 'sexual', 'spam', 'other'];
export type PackReport = { reason: string | null; details: string | null; created_at: string };
export type ReviewSuggestion = { id: number; text: string; kind: PackKind; lang: string; status: string; up: number; down: number; author: string };
export type Review = { packs: (PackItem & { reports: PackReport[] })[]; suggestions: ReviewSuggestion[] };

export const packKeys = {
  all: ['packs'] as const,
  list: (sort: PackSort, tag: string | null, q: string) => ['packs', 'list', sort, tag ?? '', q] as const,
  detail: (id: number) => ['packs', 'detail', id] as const,
  mine: ['packs', 'mine'] as const,
  creator: (id: number) => ['packs', 'creator', id] as const,
  me: ['community', 'me'] as const,
  review: ['admin', 'review'] as const,
};

export const usePackList = (sort: PackSort, tag: string | null, q: string) =>
  useQuery({
    queryKey: packKeys.list(sort, tag, q),
    queryFn: () => {
      const params = new URLSearchParams({ sort });
      if (tag) params.set('tag', tag);
      if (q) params.set('q', q);
      return api<PackItem[]>(`/packs?${params}`);
    },
    placeholderData: keepPreviousData,
  });

export const usePackDetail = (id: number) =>
  useQuery({ queryKey: packKeys.detail(id), queryFn: () => api<PackDetail>(`/packs/${id}`), enabled: Number.isInteger(id) && id > 0 });

export const useMyPacks = () => useQuery({ queryKey: packKeys.mine, queryFn: () => api<PackItem[]>('/packs/mine') });

export const useCreator = (id: number) =>
  useQuery({ queryKey: packKeys.creator(id), queryFn: () => api<Creator>(`/creators/${id}`), enabled: Number.isInteger(id) && id > 0 });

export const useCommunityMe = () => useQuery({ queryKey: packKeys.me, queryFn: () => api<CommunityMe>('/community/me'), staleTime: 60_000 });

export const useReview = (enabled: boolean) => useQuery({ queryKey: packKeys.review, queryFn: () => api<Review>('/admin/review'), enabled });

/** Server error codes of publish / edit, as messages. */
const ERRORS: Record<string, TKey> = {
  bad_pack: 'packs.errors.bad_pack',
  pack_size: 'packs.errors.pack_size',
  too_many_packs: 'packs.errors.too_many_packs',
  bad_tags: 'packs.errors.bad_tags',
  bad_language: 'packs.errors.bad_language',
  pack_removed: 'packs.errors.pack_removed',
  owner_only: 'packs.ownerOnly',
};
export const packError = (code: string): TKey => ERRORS[code] ?? 'common.failed';

/** The pack's public page (preview card in chats, "install in a room" button). */
export const packLink = (id: number) => `${API_URL || (typeof location !== 'undefined' ? location.origin : '')}/p/${id}`;
