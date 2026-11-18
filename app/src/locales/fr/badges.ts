/** Titles and badges: earned from play, one picked as my title (names and hints come from the server, already in my language). */
export const badges = {
  title: 'Titres et badges',
  /** Profile row and the screen's header. */
  count: '{earned} / {total} débloqués',
  shown: 'Titre affiché',
  noTitle: 'Aucun titre affiché',
  noTitleHint: 'Débloque un badge, puis choisis-le comme titre.',
  tiers: { bronze: 'Bronze', silver: 'Argent', gold: 'Or' },
  locked: 'À débloquer',
  progress: '{value} / {goal}',
  maxed: 'Niveau max',
  show: 'Afficher ce titre',
  hide: 'Retirer ce titre',
  shownToast: 'Titre affiché',
  hiddenToast: 'Titre retiré',
  /** Screen readers. */
  a11y: {
    medal: '{name}, {tier}',
    medalLocked: '{name}, à débloquer. {hint}',
    chip: 'Titre : {name}, {tier}',
    displayed: 'affiché comme titre',
    tiers: 'Niveau {tier} sur 3',
  },
  /** Stats profile sheet: someone's earned badges. */
  theirs: 'Badges',
  unavailable: 'Les badges ne sont pas encore disponibles sur ce serveur.',
};
