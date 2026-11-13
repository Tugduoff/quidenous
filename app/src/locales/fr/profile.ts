export const profile = {
  name: 'Prénom',
  badName: 'Prénom invalide',
  saveName: 'OK',
  photo: {
    title: 'Photo de profil',
    change: 'Changer la photo',
    avatar: 'Avatar',
    library: 'Galerie',
    camera: 'Appareil photo',
    cameraDenied: 'Accès à l’appareil photo refusé',
    /** `reason`: an HTTP status or a short error message. */
    failed: 'Photo non envoyée ({reason})',
    error: 'erreur',
  },
  theme: {
    system: 'Automatique',
    light: 'Clair',
    dark: 'Sombre',
  },
  accentA11y: 'Couleur {name}',
  accents: {
    lime: 'Citron',
    rose: 'Rose',
    bleu: 'Bleu',
    orange: 'Orange',
    violet: 'Violet',
    menthe: 'Menthe',
  },
  /** The two languages keep their own name in both UIs. */
  language: {
    title: 'Langue',
    auto: 'Automatique',
    fr: 'Français',
    en: 'English',
  },
  friends: 'Amis',
  notifications: 'Notifications',
  security: 'Sécurité',
  signOut: 'Se déconnecter',
};
