import type { Messages } from '../fr';

export const profile: Messages['profile'] = {
  name: 'First name',
  badName: 'Invalid name',
  saveName: 'OK',
  photo: {
    title: 'Profile photo',
    change: 'Change photo',
    avatar: 'Avatar',
    library: 'Gallery',
    camera: 'Camera',
    cameraDenied: 'Camera access denied',
    failed: 'Photo not sent ({reason})',
    error: 'error',
  },
  theme: {
    system: 'Automatic',
    light: 'Light',
    dark: 'Dark',
  },
  accentA11y: '{name} colour',
  accents: {
    lime: 'Lime',
    rose: 'Pink',
    bleu: 'Blue',
    orange: 'Orange',
    violet: 'Purple',
    menthe: 'Mint',
  },
  language: {
    title: 'Language',
    auto: 'Automatic',
    fr: 'Français',
    en: 'English',
  },
  friends: 'Friends',
  notifications: 'Notifications',
  security: 'Security',
  signOut: 'Sign out',
};
