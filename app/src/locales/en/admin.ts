import type { Messages } from '../fr';

export const admin: Messages['admin'] = {
  title: 'Moderation',
  empty: 'Nothing to review',
  packs: 'Packs in review',
  suggestions: 'Suggestions',
  reports: { one: '{count} report', other: '{count} reports' },
  approve: 'Approve',
  remove: 'Remove',
  reject: 'Reject',
  removeTitle: 'Remove this pack',
  reason: 'Reason (shown to the author)',
  votes: '+{up} / −{down} · {author}',
};
