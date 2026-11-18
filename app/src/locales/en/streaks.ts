import type { Messages } from '@/locales/fr';

export const streaks: Messages['streaks'] = {
  a11y: { one: '{count}-day streak', other: '{count}-day streak' },
  roomA11y: { one: 'Room streak: {count} day', other: 'Room streak: {count} days' },
  mine: { one: '{count} day in a row', other: '{count} days in a row' },
  best: 'best {count}',
  notif: 'Streak at risk',
  stats: {
    section: 'Days in a row',
    member: 'Most days in a row',
    room: 'Room streak',
    roomName: 'The whole room',
    days: { one: 'day', other: 'days' },
    memberQuip: 'Never missed a day.',
    roomQuip: 'Everyone voted, every single day.',
    now: { one: 'Still going: {count} day.', other: 'Still going: {count} days.' },
  },
  party: {
    unit: 'days in a row!',
    close: '{count} days in a row! Tap to close',
    s7: 'A whole week without missing a day.',
    s30: "A full month. It's a habit now.",
    s100: 'A hundred days. Nothing can stop you now.',
  },
};
