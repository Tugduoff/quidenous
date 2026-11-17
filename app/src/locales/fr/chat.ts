/** The conversation under a result. */
export const chat = {
  count: { one: '{count} message', other: '{count} messages' },
  longPressHint: 'Appui long : signaler ou bloquer',
  longPressOwnHint: 'Appui long : supprimer',
  seeProfile: 'Voir son profil',
  mention: 'Mentionner {name}',
  mentionAll: 'Mentionner tout le salon',
  everyoneRow: 'Tout le salon',
  /** The whole-room mention, typed after `@`. Both languages' words ping everyone whatever the app's language. */
  everyone: 'tous',
  inputLabel: 'Écrire un message',
  placeholder: 'Un avis sur ce résultat ?',
  reactions: {
    /** Bubble hints when the server has reactions. */
    hint: 'Appui long : réagir ou signaler',
    ownHint: 'Appui long : réagir ou supprimer',
    ownOnlyHint: 'Appui long : réagir',
    react: 'Réagir avec {emoji}',
    chip: { one: '{emoji}, {count} réaction', other: '{emoji}, {count} réactions' },
    chipMine: { one: '{emoji}, ta réaction', other: '{emoji}, {count} réactions, dont la tienne' },
    chipHint: 'Appui long : qui a réagi',
    count: { one: '{count} réaction', other: '{count} réactions' },
  },
  media: {
    /** The composer's button (screen readers). */
    attach: 'Envoyer une photo ou un GIF',
    library: 'Galerie',
    camera: 'Appareil photo',
    gif: 'GIF',
    caption: 'Ajouter une légende',
    captionLabel: 'Légende',
    /** Screen readers, on a message. */
    photoBy: 'Photo de {name}',
    gifBy: 'GIF de {name}',
    open: 'Toucher : agrandir',
    sending: 'Envoi en cours',
    failed: 'Pas envoyé, toucher pour réessayer',
    discard: 'Retirer',
    unreadable: 'Image illisible',
    /** KLIPY's attribution rule: its name in the search field. */
    search: 'Rechercher sur KLIPY',
    noGifs: 'Aucun GIF',
    share: 'Partager',
    save: 'Enregistrer',
  },
};
