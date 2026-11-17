import type { Messages } from '../fr';

export const chat: Messages['chat'] = {
  count: { one: '{count} message', other: '{count} messages' },
  longPressHint: 'Long press: report or block',
  longPressOwnHint: 'Long press: delete',
  seeProfile: 'See their profile',
  mention: 'Mention {name}',
  mentionAll: 'Mention the whole room',
  everyoneRow: 'The whole room',
  everyone: 'everyone',
  inputLabel: 'Write a message',
  placeholder: 'Thoughts on this result?',
  reactions: {
    hint: 'Long press: react or report',
    ownHint: 'Long press: react or delete',
    ownOnlyHint: 'Long press: react',
    react: 'React with {emoji}',
    chip: { one: '{emoji}, {count} reaction', other: '{emoji}, {count} reactions' },
    chipMine: { one: '{emoji}, your reaction', other: '{emoji}, {count} reactions including yours' },
    chipHint: 'Long press: see who reacted',
    count: { one: '{count} reaction', other: '{count} reactions' },
  },
};
