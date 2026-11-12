import type { Member } from './api';

/** The whole-room token: pings everyone in the room. */
export const EVERYONE = 'tous';

/** Lower case, no accents: "Éloïse" → "eloise". */
export const fold = (s: string) => (typeof s.normalize === 'function' ? s.normalize('NFD').replace(/[\u0300-\u036f]/g, '') : s).toLowerCase();

const isSpace = (c: string | undefined) => c == null || /\s/.test(c);
/** Part of a name: a letter (any script), a digit, `-`, `_` or an apostrophe. Ends a token when it isn't. */
const isNameChar = (c: string | undefined) => c != null && (c.toLowerCase() !== c.toUpperCase() || /[0-9_'’-]/.test(c));

/** The `@query` being typed right before the caret (no space in it), or null. `at` is the index of the `@`. */
export function activeMention(text: string, caret: number): { at: number; query: string } | null {
  const end = Math.min(caret, text.length);
  for (let i = end - 1; i >= 0 && end - i <= 40; i--) {
    const c = text[i];
    if (c === '@') return isSpace(text[i - 1]) ? { at: i, query: text.slice(i + 1, end) } : null;
    if (isSpace(c)) return null;
  }
  return null;
}

/** A name matches when it, or one of its words, starts with the query (case and accents ignored). */
export function matches(name: string, query: string) {
  const q = fold(query);
  if (!q) return true;
  const n = fold(name);
  return n.startsWith(q) || n.split(/[\s'’.-]+/).some((w) => w.startsWith(q));
}

/** Where `token` (e.g. "@Louis") stands on its own in `text`: after a space or the start, not followed by more of a name. */
function tokenAt(text: string, token: string, i: number) {
  return text.slice(i, i + token.length).toLowerCase() === token.toLowerCase() && isSpace(text[i - 1]) && !isNameChar(text[i + token.length]);
}

export function hasToken(text: string, token: string) {
  for (let i = text.indexOf('@'); i >= 0; i = text.indexOf('@', i + 1)) if (tokenAt(text, token, i)) return true;
  return false;
}

/**
 * Who a message about to be sent pings: the people picked from the list whose `@Name` is still in the text,
 * plus anyone whose full name was typed by hand (when no one else in the room has that name).
 */
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
  return { mentions: [...ids], everyone: hasToken(text, `@${EVERYONE}`) };
}

export type Segment = { text: string; member?: Member; everyone?: boolean };

/** Splits a message into plain text and mentions. Longest names first, so "@Louis-Marie" isn't read as "@Louis". */
export function segments(text: string, mentioned: Member[], everyone: boolean): Segment[] {
  const tokens: { token: string; member?: Member }[] = mentioned.filter((m) => m.name.trim()).map((m) => ({ token: `@${m.name}`, member: m }));
  if (everyone) tokens.push({ token: `@${EVERYONE}` });
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
