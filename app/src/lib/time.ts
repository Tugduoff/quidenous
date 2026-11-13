/** "il y a 5 min", "hier", "12 nov." */
export function ago(iso: string) {
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 60) return "à l'instant";
  if (s < 3600) return `il y a ${Math.floor(s / 60)} min`;
  if (s < 86400) return `il y a ${Math.floor(s / 3600)} h`;
  if (s < 172800) return 'hier';
  return new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' }).format(new Date(iso));
}

/** Time left until `iso`: "2 h 05", "14 min", "1 min" (never below a minute). */
export function until(iso: string, now = Date.now()) {
  const min = Math.max(1, Math.ceil((new Date(iso).getTime() - now) / 60000));
  if (min < 60) return `${min} min`;
  return `${Math.floor(min / 60)} h ${String(min % 60).padStart(2, '0')}`;
}
