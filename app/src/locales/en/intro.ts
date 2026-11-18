import type { Messages } from '@/locales/fr';

export const intro: Messages['intro'] = {
  skip: 'Skip',
  next: 'Next',
  step: 'Step {n} of {total}',
  hello: {
    title: "Hi, we're la bande.",
    body: 'Every day, one question about your group of friends. Everyone votes. Nobody is safe.',
    cta: 'Want to try one?',
  },
  names: { zeste: 'Zeste', bulle: 'Bulle', pave: 'Pavé', paillette: 'Paillette', roupi: 'Roupi' },
  vote: {
    eyebrow: 'Practice question',
    question: "Who'd be the first to get caught lying?",
    hint: 'Pick someone from the gang.',
    counting: '{n} of 6 voted…',
    pickA11y: 'Vote for {name}',
  },
  verdict: {
    eyebrow: 'The verdict',
    right: "Called it. It's always Bulle.",
    wrong: 'Nope: the gang said Bulle.',
    body: 'For real, the result drops once everyone has voted. Then the debate starts.',
    you: 'You',
    votesA11y: { one: '{name}: {count} vote, {pct}%', other: '{name}: {count} votes, {pct}%' },
  },
  end: {
    title: 'Your turn.',
    body: "Create a room for your friends, or join theirs with their code.",
    cta: 'Create or join a room',
    inTitle: 'Your friends are waiting.',
    inBody: "Today's question is already there.",
    inCta: "Let's go",
  },
};
