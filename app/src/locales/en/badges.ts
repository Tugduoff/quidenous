import type { Messages } from '@/locales/fr';

export const badges: Messages['badges'] = {
  title: 'Titles & badges',
  count: '{earned} / {total} unlocked',
  shown: 'Your title',
  noTitle: 'No title shown',
  noTitleHint: 'Unlock a badge, then pick it as your title.',
  tiers: { bronze: 'Bronze', silver: 'Silver', gold: 'Gold' },
  locked: 'Locked',
  progress: '{value} / {goal}',
  maxed: 'Maxed out',
  show: 'Show this title',
  hide: 'Remove this title',
  shownToast: 'Title shown',
  hiddenToast: 'Title removed',
  a11y: {
    medal: '{name}, {tier}',
    medalLocked: '{name}, locked. {hint}',
    chip: 'Title: {name}, {tier}',
    displayed: 'shown as your title',
    tiers: 'Level {tier} of 3',
  },
  theirs: 'Badges',
  unavailable: "Badges aren't available on this server yet.",
};
