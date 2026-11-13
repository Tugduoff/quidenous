import { dateFormat, t } from './i18n';

/** "il y a 5 min", "hier", "12 nov." (in the current language). */
export function ago(iso: string) {
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 60) return t('common.time.justNow');
  if (s < 3600) return t('common.time.minutesAgo', { count: Math.floor(s / 60) });
  if (s < 86400) return t('common.time.hoursAgo', { count: Math.floor(s / 3600) });
  if (s < 172800) return t('common.time.yesterday');
  return dateFormat({ day: 'numeric', month: 'short' }).format(new Date(iso));
}

/** Time left until `iso`: "2 h 05", "14 min", "1 min" (never below a minute). */
export function until(iso: string, now = Date.now()) {
  const min = Math.max(1, Math.ceil((new Date(iso).getTime() - now) / 60000));
  if (min < 60) return t('common.time.minutes', { count: min });
  return t('common.time.hoursMinutes', { h: Math.floor(min / 60), mm: String(min % 60).padStart(2, '0') });
}
