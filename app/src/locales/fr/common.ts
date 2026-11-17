/** Words used all over the app. Screen-specific text goes in that screen's namespace. */
export const common = {
  cancel: 'Annuler',
  close: 'Fermer',
  back: 'Retour',
  delete: 'Supprimer',
  confirm: 'Confirmer',
  send: 'Envoyer',
  add: 'Ajouter',
  share: 'Partager',
  copy: 'Copier',
  copied: 'Copié',
  join: 'Rejoindre',
  leave: 'Quitter',
  save: 'Enregistrer',
  continue: 'Continuer',
  retry: 'Réessayer',
  /** Full-screen fallback when a screen crashes (ErrorBoundary). */
  crashed: 'Oups, quelque chose a planté',
  offline: 'Pas de connexion',
  /** Thin banner at the top of every screen while the live connection to the server keeps failing. */
  unreachable: 'Serveur injoignable',
  failed: 'Raté, réessaie',
  /** A screen whose data didn't load (the server answered with an error). */
  loadFailed: 'Chargement impossible',
  /** "plus tard", "dans 30 s", "dans 2 min" after "réessaie". */
  later: 'plus tard',
  inSeconds: 'dans {count} s',
  inMinutes: 'dans {count} min',
  tooManyTries: "Trop d'essais, réessaie {when}",
  time: {
    justNow: "à l'instant",
    minutesAgo: 'il y a {count} min',
    hoursAgo: 'il y a {count} h',
    yesterday: 'hier',
    minutes: '{count} min',
    hoursMinutes: '{h} h {mm}',
  },
  members: { one: '{count} membre', other: '{count} membres' },
  /** In place of the name of someone whose account was deleted. */
  deletedAccount: 'Compte supprimé',
  /** Picking several items of a list to act on them at once (components/selection.tsx). */
  select: {
    start: 'Sélectionner',
    count: { one: '{count} sélectionné', other: '{count} sélectionnés' },
    all: 'Tout sélectionner',
    none: 'Tout désélectionner',
    cancel: 'Annuler la sélection',
    /** Screen readers, on a row while selecting. */
    item: 'Sélectionner {name}',
    /** After a batch action where some items could not be done. */
    partial: '{done} sur {count} faits, réessaie pour le reste',
  },
};
