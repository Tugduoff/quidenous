import type { Messages } from '../fr';

export const share: Messages['share'] = {
  friend: 'Add me as a friend on qui de nous ? 👉 {link}\n(friend code: {code})',
  invite: 'Join “{room}” on qui de nous ? 👉 {link}\n(code: {code})',
  question: 'qui de nous ? · question #{number}\n“{question}”\nCome vote 👉 {url}',
  ranking: 'qui de nous ? · ranking #{number}\n“{question}”\n{lines}\n{url}',
  result: 'qui de nous ? · result #{number}\n“{question}”\n{lines}\n{url}',
  points: '{points} pts',
  story: 'qui de nous ? · {title}\n{line}',
  historyFile: 'qui-de-nous-history.csv',
};
