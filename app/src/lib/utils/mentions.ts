import { en } from '@/locales/en';
import { fr } from '@/locales/fr';
import { t } from '@/lib/i18n';
import type { Member } from '@/lib/api/group';

export const everyone = () => t('chat.everyone');
const EVERYONE_WORDS = [...new Set([fr.chat.everyone, en.chat.everyone])];

const fold = (s: string) => (typeof s.normalize === 'function' ? s.normalize('NFD').replace(/[\u0300-\u036f]/g, '') : s).toLowerCase();

const isSpace = (c: string | undefined) => c == null || /\s/.test(c);
const isNameChar = (c: string | undefined) => c != null && (c.toLowerCase() !== c.toUpperCase() || /[0-9_'’-]/.test(c));

export function activeMention(text: string, caret: number): { at: number; query: string } | null {
  const end = Math.min(caret, text.length);
  for (let i = end - 1; i >= 0 && end - i <= 40; i--) {
    const c = text[i];
    if (c === '@') return isSpace(text[i - 1]) ? { at: i, query: text.slice(i + 1, end) } : null;
    if (isSpace(c)) return null;
  }
  return null;
}

export function matches(name: string, query: string) {
  const q = fold(query);
  if (!q) return true;
  const n = fold(name);
  return n.startsWith(q) || n.split(/[\s'’.-]+/).some((w) => w.startsWith(q));
}

function tokenAt(text: string, token: string, i: number) {
  return text.slice(i, i + token.length).toLowerCase() === token.toLowerCase() && isSpace(text[i - 1]) && !isNameChar(text[i + token.length]);
}

function hasToken(text: string, token: string) {
  for (let i = text.indexOf('@'); i >= 0; i = text.indexOf('@', i + 1)) if (tokenAt(text, token, i)) return true;
  return false;
}

export function outgoing(text: string, picked: number[], members: Member[]) {
  const ids = new Set<number>();
  for (const id of picked) {
    const m = members.find((x) => x.id === id);
    if (m && hasToken(text, `@${m.name}`)) ids.add(id);
  }
  for (const m of members) {
    if (!m.name.trim()) continue;
    const twins = members.filter((x) => fold(x.name) === fold(m.name)).length;
    if (twins === 1 && hasToken(text, `@${m.name}`)) ids.add(m.id);
  }
  return { mentions: [...ids], everyone: EVERYONE_WORDS.some((w) => hasToken(text, `@${w}`)) };
}

export type Segment = { text: string; member?: Member; everyone?: boolean };

export function segments(text: string, mentioned: Member[], everyone: boolean): Segment[] {
  const tokens: { token: string; member?: Member }[] = mentioned.filter((m) => m.name.trim()).map((m) => ({ token: `@${m.name}`, member: m }));
  if (everyone) for (const w of EVERYONE_WORDS) tokens.push({ token: `@${w}` });
  if (!tokens.length) return [{ text }];
  tokens.sort((a, b) => b.token.length - a.token.length);
  const out: Segment[] = [];
  let plain = 0;
  for (let i = text.indexOf('@'); i >= 0; i = text.indexOf('@', i + 1)) {
    const hit = tokens.find((t) => tokenAt(text, t.token, i));
    if (!hit) continue;
    if (i > plain) out.push({ text: text.slice(plain, i) });
    const end = i + hit.token.length;
    out.push({ text: text.slice(i, end), member: hit.member, everyone: !hit.member });
    plain = end;
    i = end - 1;
  }
  if (plain < text.length) out.push({ text: text.slice(plain) });
  return out;
}
