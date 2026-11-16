import type { Messages } from '../fr';

export const inbox: Messages['inbox'] = {
  title: 'Activity',
  empty: 'Nothing new.',
  mentioned: '{name} mentioned you',
  kinds: {
    message: 'New message',
    mention: 'Mention',
    nudge: 'Nudge',
    result: 'Result is in',
    question: 'New question',
    added: 'Added to a room',
    ask: 'Questions',
    friendRequest: 'Friend request',
    invite: 'Room invite',
    other: 'Notification',
  },
};
