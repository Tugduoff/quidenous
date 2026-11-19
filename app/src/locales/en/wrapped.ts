import type { Messages } from '@/locales/fr';

export const wrapped: Messages['wrapped'] = {
  entry: 'Your year in Qui de nous',
  entryLine: 'Your {year} highlights, card by card.',
  preview: 'Preview',
  adminPreview: 'Wrapped preview',
  adminPreviewLine: 'See the cards before December 1.',
  scope: { label: 'Whose Wrapped', me: 'Me', room: 'This room' },
  close: 'Close',
  prev: 'Previous card',
  next: 'Next card',
  page: 'Card {n} of {total}',
  share: 'Share',
  linkCopied: 'Link copied',
  shareText: 'qui de nous ? · {title}\n{big} · {line}',
  unavailable: 'Not yet!',
  unavailableLine: 'Your year in Qui de nous arrives on {date}.',
  empty: 'Not enough games to tell the story of your year.',
  emptyRoom: "This room hasn't played enough this year.",
  emptyLine: 'Come back after a few more questions.',
  noServer: "Your year in Qui de nous isn't available on this server yet.",
};
