import { dateFormat, t as translate } from '@/lib/i18n';
import type { HistoryItem } from '@/lib/api/history';

const dayKey = (d: Date) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;

/** "Aujourd'hui", "Hier", else "dimanche 8 novembre" (with the year when it is not this one). */
function dayLabel(d: Date) {
  const now = new Date();
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (dayKey(d) === dayKey(now)) return translate('history.today');
  if (dayKey(d) === dayKey(yesterday)) return translate('history.yesterday');
  const sameYear = d.getFullYear() === now.getFullYear();
  const label = dateFormat({ weekday: 'long', day: 'numeric', month: 'long', ...(sameYear ? {} : { year: 'numeric' }) }).format(d);
  return label[0].toUpperCase() + label.slice(1);
}

/** Consecutive items closed on the same local day (the list is newest first). */
export function byDay(items: HistoryItem[]) {
  const days: { key: string; label: string; items: HistoryItem[] }[] = [];
  for (const item of items) {
    const d = new Date(item.closed_at);
    const key = dayKey(d);
    const last = days.at(-1);
    if (last?.key === key) last.items.push(item);
    else days.push({ key, label: dayLabel(d), items: [item] });
  }
  return days;
}
