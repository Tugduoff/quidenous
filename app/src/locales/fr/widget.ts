/** The home-screen widget (docs/WIDGET.md). Short: a small widget fits about 16 characters a line. */
export const widget = {
  /** The open question waits for my vote. */
  vote: 'À toi de voter !',
  /** I voted: how many the room still waits for. */
  wait: { one: 'Plus que {count} vote', other: 'Plus que {count} votes' },
  /** A result I haven't seen. */
  result: 'Le résultat est tombé !',
  results: { one: '{count} résultat à voir', other: '{count} résultats à voir' },
  /** Under a result: the next question is open and waits for me. */
  nextToo: "Une nouvelle question t'attend",
  /** No open question (paused room, or out of questions). */
  idle: "Pas de question pour l'instant",
  /** A paused room: when the next one opens by itself. */
  idleAt: 'Prochaine à {time}',
  /** Signed out, or in no room. */
  open: 'Ouvre qui de nous ?',
  signedOut: 'Connecte-toi pour jouer',
  noRoom: 'Rejoins un salon pour jouer',
  /** Couldn't load and nothing saved yet. */
  loading: 'La question arrive…',
};
