/** Live reactions on a result (docs/REACTIONS.md). */
export const reactions = {
  /** The bar under a result, for screen readers. */
  bar: 'Réactions',
  /** One emoji of the bar: what a tap does, and its count. */
  button: { one: 'Réagir avec {emoji}, {count} réaction', other: 'Réagir avec {emoji}, {count} réactions' },
  mine: 'dont {count} de toi',
  who: 'Appui long pour voir qui a réagi',
  /** My 30 taps on this result are used up. */
  capped: 'Plus de réactions possibles sur ce résultat',
  /** The who-reacted dialog's title, next to the emoji. */
  count: { one: '{count} réaction', other: '{count} réactions' },
  /** Big screen: the counts of the result. */
  screen: { one: '{emoji} : {count} réaction', other: '{emoji} : {count} réactions' },
};
