import { Fragment, type ReactNode } from 'react';
import { useLang } from '@/lib/i18n';

export function SameLanguage({ children }: { children: ReactNode }) {
  const { lang } = useLang();
  return <Fragment key={lang}>{children}</Fragment>;
}
