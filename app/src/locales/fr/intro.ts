/** The first-run intro (app/intro.tsx): la bande shows a new account how a round goes. Short: the mascots do the talking. */
export const intro = {
  skip: 'Passer',
  next: 'Suivant',
  /** Screen readers: "Étape 2 sur 3". */
  step: 'Étape {n} sur {total}',
  hello: {
    title: "Salut, nous, c'est la bande.",
    body: "Chaque jour, une question sur ta bande d'amis. Tout le monde vote. Personne n'est à l'abri.",
    cta: 'On fait un essai ?',
  },
  names: { zeste: 'Zeste', bulle: 'Bulle', pave: 'Pavé', paillette: 'Paillette', roupi: 'Roupi' },
  vote: {
    eyebrow: "Question d'essai",
    question: 'Qui se ferait griller en premier en mentant ?',
    hint: "Choisis quelqu'un de la bande.",
    /** While the others' votes come in. */
    counting: '{n} sur 6 ont voté…',
    /** Screen readers, on a candidate. */
    pickA11y: 'Voter pour {name}',
  },
  verdict: {
    eyebrow: 'Le verdict',
    right: "Bien vu. C'est toujours Bulle.",
    wrong: 'Raté : la bande a dit Bulle.',
    body: 'Pour de vrai, le résultat tombe quand tout le monde a voté. Ensuite, place au débat.',
    you: 'Toi',
    votesA11y: { one: '{name} : {count} vote, {pct} %', other: '{name} : {count} votes, {pct} %' },
  },
  end: {
    /** No room yet: on to create or join one. */
    title: 'À ton tour.',
    body: 'Crée un salon pour ta bande, ou rejoins celui de tes amis avec leur code.',
    cta: 'Créer ou rejoindre un salon',
    /** Already in a room (joined from an invite link). */
    inTitle: "Ta bande t'attend.",
    inBody: 'La question du jour est déjà là.',
    inCta: "C'est parti",
  },
};
