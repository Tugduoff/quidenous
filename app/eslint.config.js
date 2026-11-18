const { defineConfig } = require('eslint/config');
const comments = require('@eslint-community/eslint-plugin-eslint-comments');
const expo = require('eslint-config-expo/flat');

const SRC = ['src/**/*.{ts,tsx}', 'index.ts'];

const RELATIVE = {
  group: ['./*', '../*', '.', '..'],
  message: 'Import through the @/ alias (e.g. @/lib/api/rounds), never a relative path.',
};
const RAW_CLIENT = {
  name: '@/lib/api/client',
  importNames: ['api', 'apiUpload', 'fetchExternal'],
  message: 'Only src/lib/api/** calls the server. Import a named function or hook from @/lib/api/<domain> (add one there if missing).',
};
const NO_APP = {
  group: ['@/app', '@/app/*'],
  message: 'Routes are leaves: nothing imports from src/app. Move the shared code to src/components or src/lib.',
};
const NO_COMPONENTS = {
  group: ['@/components', '@/components/*'],
  message: 'src/lib never imports UI. Move the logic into src/lib, or pass it in from the component.',
};
const UTILS_PURE = {
  group: ['react', 'react-native', 'react-native-*', 'expo', 'expo-*', '@tanstack/*', '@/lib/api/*', '@/lib/device/*', '@/lib/hooks/*', '@/lib/session'],
  allowTypeImports: true,
  message: 'src/lib/utils holds pure functions (no React, no I/O). Put hooks in src/lib/hooks, server calls in src/lib/api, device I/O in src/lib/device.',
};
const NO_HAPTICS = {
  name: 'expo-haptics',
  message: 'Use impactHaptic / selectionHaptic from @/lib/device/haptics.',
};

const restrict = (patterns, paths = []) => ['error', { patterns, paths }];

const NO_FETCH = {
  name: 'fetch',
  message: 'Only src/lib/api/** talks to the network. Use a named function from @/lib/api/<domain> (readUriBlob / readUriText for local files).',
};
const NO_FETCH_PROPERTY = ['window', 'globalThis', 'self', 'global'].map((object) => ({
  object,
  property: 'fetch',
  message: NO_FETCH.message,
}));

module.exports = defineConfig([
  expo,
  { ignores: ['dist/*', '.expo/*', 'node_modules/*'] },
  { linterOptions: { reportUnusedDisableDirectives: 'error' } },
  {
    files: ['scripts/**/*.mjs'],
    languageOptions: { globals: { Buffer: 'readonly', process: 'readonly', console: 'readonly', Bun: 'readonly' } },
  },
  {
    rules: {
      'react-hooks/refs': 'warn',
      'react-hooks/immutability': 'warn',
      'react-hooks/set-state-in-effect': 'warn',
      'react-hooks/static-components': 'warn',
      'react-hooks/purity': 'warn',
    },
  },
  {
    files: SRC,
    plugins: { '@eslint-community/eslint-comments': comments },
    rules: {
      '@eslint-community/eslint-comments/no-restricted-disable': [
        'error',
        '@typescript-eslint/no-explicit-any',
        '@typescript-eslint/no-non-null-assertion',
        '@typescript-eslint/no-restricted-imports',
        'react-hooks/exhaustive-deps',
        'no-restricted-globals',
        'no-restricted-properties',
      ],
      '@eslint-community/eslint-comments/no-unlimited-disable': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-non-null-assertion': 'error',
      'react-hooks/exhaustive-deps': 'error',
      'no-restricted-imports': 'off',
      '@typescript-eslint/no-restricted-imports': restrict([RELATIVE], [RAW_CLIENT, NO_HAPTICS]),
      'no-restricted-globals': ['error', NO_FETCH],
      'no-restricted-properties': ['error', ...NO_FETCH_PROPERTY],
    },
  },
  {
    files: ['src/app/**/*.{ts,tsx}'],
    rules: { '@typescript-eslint/no-restricted-imports': restrict([RELATIVE, NO_APP], [RAW_CLIENT, NO_HAPTICS]) },
  },
  {
    files: ['src/components/**/*.{ts,tsx}'],
    rules: { '@typescript-eslint/no-restricted-imports': restrict([RELATIVE, NO_APP], [RAW_CLIENT, NO_HAPTICS]) },
  },
  {
    files: ['src/lib/**/*.{ts,tsx}'],
    rules: { '@typescript-eslint/no-restricted-imports': restrict([RELATIVE, NO_APP, NO_COMPONENTS], [RAW_CLIENT, NO_HAPTICS]) },
  },
  {
    files: ['src/lib/utils/**/*.ts'],
    ignores: ['src/lib/utils/**/*.test.ts'],
    rules: { '@typescript-eslint/no-restricted-imports': restrict([RELATIVE, NO_APP, NO_COMPONENTS, UTILS_PURE], [RAW_CLIENT, NO_HAPTICS]) },
  },
  {
    files: ['src/lib/device/**/*.{ts,tsx}'],
    rules: { '@typescript-eslint/no-restricted-imports': restrict([RELATIVE, NO_APP, NO_COMPONENTS], [RAW_CLIENT]) },
  },
  {
    files: ['src/lib/api/**/*.{ts,tsx}'],
    rules: {
      '@typescript-eslint/no-restricted-imports': restrict([RELATIVE, NO_APP, NO_COMPONENTS], [NO_HAPTICS]),
      'no-restricted-globals': 'off',
      'no-restricted-properties': 'off',
    },
  },
  {
    files: ['src/widget/**/*.{ts,tsx}'],
    rules: {
      '@typescript-eslint/no-restricted-imports': restrict([RELATIVE, NO_APP, NO_COMPONENTS], [RAW_CLIENT, NO_HAPTICS]),
      'no-restricted-globals': 'off',
      'no-restricted-properties': 'off',
    },
  },
]);
