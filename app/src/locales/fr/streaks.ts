/** Streaks (docs/STREAKS.md): days in a row with a vote, per person and for the whole room. */
export const streaks = {
  /** The flame and its number next to a name. */
  a11y: { one: 'Série de {count} jour', other: 'Série de {count} jours' },
  roomA11y: { one: 'Série du salon : {count} jour', other: 'Série du salon : {count} jours' },
  /** Profile, under my name. */
  mine: { one: "{count} jour d'affilée", other: "{count} jours d'affilée" },
  best: 'record {count}',
  /** Notification settings: the evening reminder. */
  notif: 'Série en danger',
  stats: {
    section: "Jours d'affilée",
    member: "Record d'assiduité",
    room: 'Série du salon',
    roomName: 'Tout le salon',
    days: { one: 'jour', other: 'jours' },
    memberQuip: 'Pas un jour sans voter.',
    roomQuip: 'Tout le monde a voté, chaque jour.',
    now: { one: 'En cours : {count} jour.', other: 'En cours : {count} jours.' },
  },
  /** The party at 7, 30 and 100 days. */
  party: {
    unit: "jours d'affilée !",
    close: "{count} jours d'affilée ! Toucher pour fermer",
    s7: 'Une semaine sans rater un seul jour.',
    s30: "Un mois entier. C'est une habitude, maintenant.",
    s100: "Cent jours. Plus rien ne t'arrête.",
  },
};
