import { getLang, t as translate } from './i18n';

/**
 * French typography for display: a no-break space before ? ! : ; » and after «, so the mark never wraps onto its own line.
 * Any run of spaces (or an existing no-break space) collapses to one.
 */
export const tidy = (s: string) => s.replace(/\s+([?!:;»])/g, ' $1').replace(/«\s+/g, '« ');

/** "1er", "2e" / "1st", "2nd", "3rd", "11th": a place in a ranking. */
export function ordinal(n: number) {
  const a = n % 10;
  const b = n % 100;
  const en = a === 1 && b !== 11 ? 'st' : a === 2 && b !== 12 ? 'nd' : a === 3 && b !== 13 ? 'rd' : 'th';
  return translate(`vote.ordinal.${getLang() === 'fr' ? (n === 1 ? 'st' : 'th') : en}`, { n });
}
