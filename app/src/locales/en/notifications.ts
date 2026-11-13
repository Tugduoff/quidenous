import type { Messages } from '../fr';

export const notifications: Messages['notifications'] = {
  title: 'Notifications',
  unavailable: 'Settings unavailable',
  blockedBrowser: 'Blocked by the browser',
  blockedSettings: 'Blocked in settings',
  enableHere: 'Turn on for this device',
  unsupported: 'Not available here',
  enableFailed: "Couldn't turn on for this device",
  notSaved: 'Setting not saved',
  tellMe: 'Notify me about',
  kinds: {
    question: 'New question',
    result: 'Result',
    chat: 'Messages',
    mention: 'Mentions',
    nudge: 'Nudges',
    added: 'Added to a room',
    ask: 'Question requests',
  },
  quiet: 'Quiet hours',
  start: 'Start',
  end: 'End',
  timeA11y: '{label}: {time}. Change',
};
