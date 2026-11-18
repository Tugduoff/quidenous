/** « Aujourd'hui »: all my rooms on one screen, what each one wants from me today (lib/today.ts). */
export const today = {
  tab: "Aujourd'hui",
  /** The big line under the date. */
  summary: {
    vote: { one: "{count} question t'attend", other: "{count} questions t'attendent" },
    results: { one: '{count} résultat à découvrir', other: '{count} résultats à découvrir' },
    /** Under `vote`, when there are results too. */
    andResults: { one: 'et {count} résultat à découvrir', other: 'et {count} résultats à découvrir' },
    done: 'Tout est à jour',
    /** Under `done`: some questions still wait for the others. */
    doneWaiting: "Tu as voté partout. Plus qu'à attendre les autres.",
    doneIdle: "Rien à faire pour l'instant. On te prévient dès que ça bouge.",
  },
  /** The line at the bottom of a card. */
  state: {
    vote: 'À toi de voter',
    result: { one: 'Résultat dispo', other: '{count} résultats dispo' },
    tripot: 'Tripot ouvert',
    wait: { one: 'En attente de {count}', other: 'En attente de {count}' },
    waitAll: 'Bientôt le résultat',
    next: 'Prochaine question dans {time}',
    none: 'Pas de question en cours',
  },
  /** My streak in the room isn't saved yet today. */
  atRisk: 'Série en jeu',
  unreadA11y: { one: '{count} message non lu', other: '{count} messages non lus' },
  votesA11y: '{votes} votes sur {needed}',
  /** Screen readers: the whole card is one button. */
  hint: {
    vote: 'Ouvre la question pour voter',
    result: 'Ouvre le résultat',
    other: 'Ouvre ce salon',
  },
  rooms: 'Tous mes salons',
};
