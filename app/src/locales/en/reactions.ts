import type { Messages } from '../fr';

export const reactions: Messages['reactions'] = {
  bar: 'Reactions',
  button: { one: 'React with {emoji}, {count} reaction', other: 'React with {emoji}, {count} reactions' },
  mine: '{count} from you',
  who: 'Long press to see who reacted',
  capped: "You're out of reactions on this result",
  count: { one: '{count} reaction', other: '{count} reactions' },
  screen: { one: '{emoji}: {count} reaction', other: '{emoji}: {count} reactions' },
};
