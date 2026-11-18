// https://docs.expo.dev/guides/using-eslint/
const { defineConfig } = require('eslint/config');
const expo = require('eslint-config-expo/flat');

module.exports = defineConfig([
  expo,
  { ignores: ['dist/*', '.expo/*', 'node_modules/*'] },
  {
    // Node and Bun scripts (OTA publishing, avatar parts, widget previews).
    files: ['scripts/**/*.mjs'],
    languageOptions: { globals: { Buffer: 'readonly', process: 'readonly', console: 'readonly', Bun: 'readonly' } },
  },
  {
    // React Compiler diagnostics: each one only makes the compiler skip that component (it stays correct, just not auto-memoised).
    // Shown as warnings; the app keeps a few deliberate patterns (refs read in render, state set from effects).
    rules: {
      'react-hooks/refs': 'warn',
      'react-hooks/immutability': 'warn',
      'react-hooks/set-state-in-effect': 'warn',
      'react-hooks/static-components': 'warn',
      'react-hooks/purity': 'warn',
    },
  },
]);
