import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { tidy } from '@/lib/i18n-core';

const NBSP = ' ';

describe('French typography', () => {
  test('a high punctuation mark never wraps alone onto the next line', () => {
    assert.equal(tidy('Qui de nous ?'), `Qui de nous${NBSP}?`);
    assert.equal(tidy('Bravo !'), `Bravo${NBSP}!`);
    assert.equal(tidy('Note : ok ; fin'), `Note${NBSP}: ok${NBSP}; fin`);
  });

  test('guillemets hold their content with no-break spaces', () => {
    assert.equal(tidy('« oui »'), `«${NBSP}oui${NBSP}»`);
  });

  test('a run of spaces or an existing no-break space collapses to one', () => {
    assert.equal(tidy(`Vraiment   ?`), `Vraiment${NBSP}?`);
    assert.equal(tidy(`Vraiment${NBSP}?`), `Vraiment${NBSP}?`);
  });

  test('a mark with no space before it is left alone', () => {
    assert.equal(tidy('https://x.fr'), 'https://x.fr');
  });
});
