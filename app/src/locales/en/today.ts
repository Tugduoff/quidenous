import type { Messages } from '../fr';

export const today: Messages['today'] = {
  tab: 'Today',
  summary: {
    vote: { one: '{count} question for you', other: '{count} questions for you' },
    results: { one: '{count} new result', other: '{count} new results' },
    andResults: { one: 'and {count} new result', other: 'and {count} new results' },
    done: 'All caught up',
    doneWaiting: "You've voted everywhere. Now it's up to the others.",
    doneIdle: "Nothing to do for now. We'll tell you when something happens.",
  },
  state: {
    vote: 'Your turn to vote',
    result: { one: "Result's in", other: '{count} results in' },
    tripot: 'Tripot is open',
    wait: { one: 'Waiting on {count}', other: 'Waiting on {count}' },
    waitAll: 'Result coming up',
    next: 'Next question in {time}',
    none: 'No question right now',
  },
  atRisk: 'Streak at stake',
  unreadA11y: { one: '{count} unread message', other: '{count} unread messages' },
  votesA11y: '{votes} of {needed} votes',
  hint: {
    vote: 'Opens the question to vote',
    result: 'Opens the result',
    other: 'Opens this room',
  },
  rooms: 'All my rooms',
};
