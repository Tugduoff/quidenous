import type { Messages } from '../fr';

export const widget: Messages['widget'] = {
  vote: 'Your turn to vote!',
  wait: { one: '{count} vote to go', other: '{count} votes to go' },
  result: 'The result is out!',
  results: { one: '{count} result to see', other: '{count} results to see' },
  nextToo: 'A new question is waiting',
  idle: 'No question right now',
  idleAt: 'Next one at {time}',
  open: 'Open qui de nous ?',
  signedOut: 'Sign in to play',
  noRoom: 'Join a room to play',
  loading: 'Question incoming…',
};
