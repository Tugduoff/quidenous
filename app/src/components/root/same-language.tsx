import { Fragment, type ReactNode } from 'react';
import { useLang } from '@/lib/i18n';

/**
 * Each screen, mounted afresh when the language changes: text that helpers (names, dates, errors) translated
 * outside `useT` is memoised by the React Compiler and would otherwise stay in the old language until a reload.
 */
export function SameLanguage({ children }: { children: ReactNode }) {
  const { lang } = useLang();
  return <Fragment key={lang}>{children}</Fragment>;
}
