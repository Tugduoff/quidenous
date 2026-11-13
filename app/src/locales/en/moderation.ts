import type { Messages } from '../fr';

export const moderation: Messages['moderation'] = {
  reasons: { offensive: 'Offensive', harassment: 'Harassment', sexual: 'Sexual content', spam: 'Spam', other: 'Other' },
  reportMessage: 'Report this message',
  reportQuestion: 'Report this question',
  reportPerson: 'Report {name}',
  report: 'Report',
  block: 'Block {name}',
  unblock: 'Unblock {name}',
  details: 'Tell us more (optional)',
  blockTitle: 'Block {name}?',
  blockBody: "You won't see their messages anymore.",
  blockConfirm: 'Block',
  done: 'Reported',
};
