import { api, apiUpload } from '@/lib/api/client';
import type { Member } from '@/lib/api/group';
import type { Lang } from '@/lib/i18n-core';
import type { LangPref } from '@/lib/i18n';

export const getMe = () => api<Member>('/me');

export const setMyName = (name: string) => api<Member>('/me', { method: 'PUT', json: { name } });

export const setMyAvatar = (avatar: string) => api<Member>('/me/avatar', { method: 'PUT', json: { avatar } });

export const uploadMyPhoto = (form: FormData) => apiUpload<Member>('/me/photo', form);

export type AccountLocale = { locale: Lang | null; pref: LangPref | null };

export const getMyLocale = () => api<AccountLocale>('/me/locale');

export const setMyLocale = (locale: Lang, pref: LangPref) => api('/me/locale', { method: 'PUT', json: { locale, pref } });
