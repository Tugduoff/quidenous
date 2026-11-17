/** The global question of the day (docs/GLOBAL_QUESTION.md): the same deck question in every room, anonymous aggregates. */
export const globalQuestion = {
  /** Small label over the question (home card, screen). */
  label: 'Question du monde',
  /** Home card, once I answered. */
  voted: 'Tu as répondu',
  /** Under the question, before voting. */
  forRoom: 'Le même jour, la même question dans tous les salons. Un seul vote, définitif.',
  answered: { one: '{count} a répondu dans le salon', other: '{count} ont répondu dans le salon' },
  myRoom: 'Dans ton salon',
  roomVoters: '{voters}/{members} ont répondu',
  everywhere: 'Tous les salons',
  /** Fewer than `min_rooms` rooms answered: no aggregates yet. */
  notEnough: 'Pas encore assez de salons',
  roomsSoFar: { one: '{count} salon sur {min} a répondu', other: '{count} salons sur {min} ont répondu' },
  rooms: { one: '{count} salon', other: '{count} salons' },
  yesterday: 'Hier',
  noVotesYesterday: 'Personne n’a répondu dans ton salon',
  facts: {
    unanimous: 'Dans {pct} des salons, tout le monde a choisi la même personne',
    owner: 'Dans {pct} des salons, c’est le propriétaire du salon',
    newest: 'Dans {pct} des salons, c’est le dernier arrivé',
    last_winner: 'Dans {pct} des salons, c’est aussi le gagnant de la dernière question',
    self: '{pct} des votants se sont choisis eux-mêmes',
    top_share: 'En moyenne, la personne en tête récolte {pct} des votes de son salon',
  },
};
