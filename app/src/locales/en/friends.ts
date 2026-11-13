import type { Messages } from '../fr';

export const friends: Messages['friends'] = {
  title: 'Friends',
  myCode: 'My friend code',
  shareCode: 'Share my code',
  linkCopied: 'Link copied',
  addWithCode: 'Add with a code',
  code: 'Friend code',
  addA11y: 'Add friend',
  notFriendQr: "This QR code isn't a friend code",
  added: '{name} is now your friend',
  errors: {
    badCode: 'Unknown friend code',
    self: "That's your own code",
  },
  count: { one: '{count} friend', other: '{count} friends' },
  friend: 'Friend',
  addedAsFriend: 'Added as a friend',
  roomsInCommon: { one: '{count} room in common', other: '{count} rooms in common' },
  addNameA11y: 'Add {name} as a friend',
  removeA11y: 'Remove {name} from my friends',
};
