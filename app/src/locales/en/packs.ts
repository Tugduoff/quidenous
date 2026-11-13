import type { Messages } from '../fr';

export const packs: Messages['packs'] = {
  title: 'Packs',
  search: 'Search packs',
  none: 'No packs',
  byline: '{count} questions · by {author}',
  publishTitle: 'Publish a pack',
  name: 'Pack name',
  description: 'Description',
  descriptionPlaceholder: 'Description (optional)',
  fromFile: 'From a file',
  publish: 'Publish',
  publishing: 'Publishing…',
  published: 'Published: {count} questions',
  errors: {
    bad_pack: 'A name (40 characters max) and a short description',
    pack_size: 'It needs 5 to 300 questions',
    too_many_packs: 'You’ve already published 10 packs',
  },
  details: '{count} questions · by {author} · {installs} installs',
  install: 'Add to “{room}”',
  installed: '+{added} questions in “{room}”',
  installedSkipped: '+{added} questions in “{room}” ({skipped} already there)',
  ownerOnly: 'Only the room owner can do that',
  reported: 'Thanks, it’s reported',
  removeMine: 'Remove my pack',
  report: 'Report',
};
