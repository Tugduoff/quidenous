/** Le Tripot (docs/TRIPOT.md): bets on who wins, with chips worth nothing. */
export const tripot = {
  name: 'Tripot',
  /** The row on the ballot and the waiting screen. */
  bet: 'Parier',
  betA11y: 'Parier des jetons sur le gagnant',
  pot: 'Pot {pot}',
  bettors: { one: '{count} parieur', other: '{count} parieurs' },
  myBet: 'Mise : {amount} sur {name}',
  myBetHidden: 'Mise : {amount}',
  broke: 'Plus de jetons ce mois-ci',
  /** The bet sheet. */
  whoPick: 'Qui va gagner ?',
  whoRank: 'Qui finira premier ?',
  amount: 'Mise',
  allIn: 'Tapis',
  chipA11y: 'Miser {amount} jetons',
  balance: { one: 'Tu as {count} jeton', other: 'Tu as {count} jetons' },
  confirm: 'Miser {amount}',
  noValue: 'Jetons sans valeur, juste pour le jeu.',
  final: 'Un seul pari par question, sans retour.',
  /** The result. */
  potOf: { one: 'Pot de {count} jeton', other: 'Pot de {count} jetons' },
  won: '+{won}',
  wonA11y: '{name} gagne {won} jetons',
  mineWon: 'Tu gagnes {won} jetons',
  mineLost: 'Mise perdue : {amount} sur {name}',
  refunded: 'Personne sur le gagnant : mises rendues',
  /** Stats. */
  board: 'Tripot',
  chips: { one: 'jeton', other: 'jetons' },
  podium: '{month} : le podium',
  reset: 'Sans valeur · retour à {start} le 1er du mois',
  /** Room settings. */
  setting: 'Tripot (paris en jetons)',
};
