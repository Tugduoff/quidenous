import { api, apiUpload } from '@/lib/api/client';
import type { Member } from '@/lib/api/group';

export const getMe = () => api<Member>('/me');

export const setMyName = (name: string) => api<Member>('/me', { method: 'PUT', json: { name } });

export const setMyAvatar = (avatar: string) => api<Member>('/me/avatar', { method: 'PUT', json: { avatar } });

export const uploadMyPhoto = (form: FormData) => apiUpload<Member>('/me/photo', form);
