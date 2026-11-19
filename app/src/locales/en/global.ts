import type { Messages } from '@/locales/fr';

export const globalQuestion: Messages['globalQuestion'] = {
  label: 'Global question',
  voted: 'You answered',
  forRoom: 'Same day, same question in every room. One vote, final.',
  answered: { one: '{count} answered in your room', other: '{count} answered in your room' },
  myRoom: 'In your room',
  roomVoters: '{voters}/{members} answered',
  everywhere: 'All rooms',
  notEnough: 'Not enough rooms yet',
  roomsSoFar: { one: '{count} of {min} rooms answered', other: '{count} of {min} rooms answered' },
  rooms: { one: '{count} room', other: '{count} rooms' },
  yesterday: 'Yesterday',
  noVotesYesterday: 'Nobody answered in your room',
  facts: {
    unanimous: 'In {pct} of rooms, everyone picked the same person',
    owner: 'In {pct} of rooms, it’s the room’s owner',
    newest: 'In {pct} of rooms, it’s the newest member',
    last_winner: 'In {pct} of rooms, it’s also who won the last question',
    self: '{pct} of voters picked themselves',
    top_share: 'On average, the top pick gets {pct} of their room’s votes',
  },
};
