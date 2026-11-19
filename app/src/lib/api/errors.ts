import { t } from '@/lib/i18n';
import { ApiError } from '@/lib/api/client';
import { errors } from '@/locales/fr/errors';
import { PASSWORD_MIN } from '@/lib/utils/credentials';

const wait = (s?: number) =>
  !s ? t('common.later') : s < 60 ? t('common.inSeconds', { count: s }) : t('common.inMinutes', { count: Math.ceil(s / 60) });

export function authError(e: unknown): string {
  if (!(e instanceof ApiError)) return t('common.offline');
  if (e.code === 'rate_limited') return t('common.tooManyTries', { when: wait(e.retryAfter) });
  return Object.prototype.hasOwnProperty.call(errors, e.code)
    ? t(`errors.${e.code as keyof typeof errors}`, { min: PASSWORD_MIN })
    : t('common.failed');
}
