import { getLang, t as translate } from '@/lib/i18n';

export { tidy } from '@/lib/i18n';

export function ordinal(n: number) {
  const a = n % 10;
  const b = n % 100;
  const en = a === 1 && b !== 11 ? 'st' : a === 2 && b !== 12 ? 'nd' : a === 3 && b !== 13 ? 'rd' : 'th';
  return translate(`vote.ordinal.${getLang() === 'fr' ? (n === 1 ? 'st' : 'th') : en}`, { n });
}
