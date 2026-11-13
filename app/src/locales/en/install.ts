import type { Messages } from '../fr';

export const install: Messages['install'] = {
  banner: 'Install the app',
  hide: 'Hide',
  title: 'Install the app',
  sub: 'For notifications and full screen.',
  desktopTitle: 'Play it on your phone',
  desktopSub: 'Scan this QR code with your phone.',
  ios: {
    share: 'Tap Share',
    addToHome: '“Add to Home Screen”',
  },
  android: {
    installed: 'Installed: open it',
    install: 'Install',
    menu: 'Open the ⋮ menu',
    tapInstall: 'Tap “Install app”',
  },
  open: 'Open the installed app',
  inviteCode: 'Invite code to enter in the app',
  friendCode: 'Friend code to enter in the app',
};
