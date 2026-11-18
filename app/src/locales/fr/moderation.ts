/** Report and block (long-press on a message, a member, a question). */
export const moderation = {
  reasons: { offensive: 'Offensant', harassment: 'Harcèlement', sexual: 'Contenu sexuel', spam: 'Spam', other: 'Autre' },
  reportMessage: 'Signaler ce message',
  reportQuestion: 'Signaler cette question',
  reportAnswer: 'Signaler cette réponse',
  reportPerson: 'Signaler {name}',
  report: 'Signaler',
  block: 'Bloquer {name}',
  unblock: 'Débloquer {name}',
  details: 'Précise (facultatif)',
  blockTitle: 'Bloquer {name} ?',
  blockBody: 'Tu ne verras plus ses messages.',
  blockConfirm: 'Bloquer',
  done: 'Signalé',
  delete: 'Supprimer',
  deleteTitle: 'Supprimer ce message ?',
  deleteConfirm: 'Supprimer',
  /** The room owner brings back a message hidden by reports. */
  restore: 'Rétablir',
};
