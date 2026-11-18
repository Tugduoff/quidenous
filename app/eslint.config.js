const { defineConfig } = require('eslint/config');
const comments = require('@eslint-community/eslint-plugin-eslint-comments');
const expo = require('eslint-config-expo/flat');

const SRC = ['src/**/*.{ts,tsx}', 'index.ts'];
const ROUTE_MAX_LINES = 150;
const FILE_MAX_LINES = 500;

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

const EFFECT_HOOKS = ['useEffect', 'useLayoutEffect', 'useInsertionEffect'];
const EFFECT_MESSAGE =
  'Raw effects live only in src/lib/hooks. Derive the value in render, act in the event handler, or use (or add) a named hook from @/lib/hooks (useMountEffect, useTimeout, useOnChange, useWhen, useAnimateOnChange, useAppStateChange...).';
const NO_EFFECTS = { name: 'react', importNames: EFFECT_HOOKS, message: EFFECT_MESSAGE };
const NO_EFFECT_MEMBERS = {
  selector: `MemberExpression[object.name='React'][property.name=/^(${EFFECT_HOOKS.join('|')})$/]`,
  message: EFFECT_MESSAGE,
};

const COLOR_MESSAGE =
  'Colours come from the theme: t.<token> from useTheme() in a component, COLORS / ACCENTS / themes from @/lib/tokens outside one. Add a named token in src/lib/tokens.ts if no existing one fits.';
const HEX = '#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})';
const NO_COLOR_LITERALS = [
  { selector: `Literal[value=/^${HEX}$/]`, message: COLOR_MESSAGE },
  { selector: 'Literal[value=/rgba?\\(/]', message: COLOR_MESSAGE },
  { selector: `TemplateElement[value.raw=/${HEX}\\b|rgba?\\(/]`, message: COLOR_MESSAGE },
];
const NO_FONT_LITERALS = {
  selector: 'Literal[value=/^(Inter|BricolageGrotesque)_/]',
  message: 'Font families come from the theme: t.font.<weight> from useTheme(), or FONT from @/lib/tokens outside a component.',
};
const TEXT_MESSAGE =
  'User-facing text goes through i18n: tr(\'<area>.<key>\') from useT() (@/lib/i18n), with the French string in src/locales/fr and the English one under the same key in src/locales/en. Language-neutral text belongs in LANGUAGE_NEUTRAL_TEXT in eslint.config.js.';
const LANGUAGE_NEUTRAL_TEXT = ['qui de nous', 'qui de nous \\?', 'BAR', 'https:..…', 'ABC234', 'xxxxx-xxxxx'];
const HAS_LETTER = '/[A-Za-zÀ-ÖØ-öø-ÿ]/';
const NEUTRAL = `/^\\s*(${LANGUAGE_NEUTRAL_TEXT.join('|')})\\s*$/`;
const WORDS = `[value=${HAS_LETTER}]:not([value=${NEUTRAL}])`;
const TEMPLATE_WORDS = `TemplateLiteral:has(> TemplateElement[value.raw=${HAS_LETTER}])`;
const TEXT_PROPS = 'JSXAttribute[name.name=/^(placeholder|title|label|accessibilityLabel|accessibilityHint|message|text)$/]';
const CONFIRM_TONES = '/^(acc|danger)$/';
const TEXT_CALLS = 'CallExpression[callee.name=/^(showToast|askConfirm)$/]';
const NO_HARD_CODED_TEXT = [
  { selector: `JSXText${WORDS}`, message: TEXT_MESSAGE },
  { selector: `:matches(JSXElement, JSXFragment) > JSXExpressionContainer > :matches(Literal${WORDS}, ${TEMPLATE_WORDS})`, message: TEXT_MESSAGE },
  { selector: `${TEXT_PROPS} > Literal${WORDS}`, message: TEXT_MESSAGE },
  { selector: `${TEXT_PROPS} > JSXExpressionContainer > :matches(Literal${WORDS}, ${TEMPLATE_WORDS})`, message: TEXT_MESSAGE },
  { selector: `${TEXT_CALLS} > :matches(Literal${WORDS}:not([value=${CONFIRM_TONES}]), ${TEMPLATE_WORDS})`, message: TEXT_MESSAGE },
];
const LITERAL_TOKEN_FILES = [
  'src/lib/tokens.ts',
  'src/components/celebration/**',
  'src/components/mascot/**',
  'src/lib/utils/mascot.ts',
  'src/components/auth/brand-icons.tsx',
  'src/**/*.test.ts',
];
const syntax = ({ effects = true, literalTokens = false } = {}) => [
  'error',
  ...(effects ? [NO_EFFECT_MEMBERS] : []),
  ...(literalTokens ? [] : [...NO_COLOR_LITERALS, NO_FONT_LITERALS]),
  ...NO_HARD_CODED_TEXT,
];

const LOCKED_RULES = [
  '@typescript-eslint/no-explicit-any',
  '@typescript-eslint/no-non-null-assertion',
  '@typescript-eslint/no-restricted-imports',
  'react-hooks/exhaustive-deps',
  'no-restricted-globals',
  'no-restricted-properties',
  'no-restricted-syntax',
  'max-lines',
];
const COMPILER_RULES = ['react-hooks/refs', 'react-hooks/immutability', 'react-hooks/set-state-in-effect', 'react-hooks/static-components', 'react-hooks/purity'];

const restrict = (patterns, paths = []) => ['error', { patterns, paths: [...paths, NO_EFFECTS] }];
const restrictInHooks = (patterns, paths = []) => ['error', { patterns, paths }];

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
      ...Object.fromEntries(COMPILER_RULES.map((rule) => [rule, 'error'])),
    },
  },
  {
    files: SRC,
    plugins: { '@eslint-community/eslint-comments': comments },
    rules: {
      '@eslint-community/eslint-comments/no-restricted-disable': ['error', ...LOCKED_RULES, ...COMPILER_RULES],
      '@eslint-community/eslint-comments/no-unlimited-disable': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-non-null-assertion': 'error',
      'react-hooks/exhaustive-deps': 'error',
      'no-restricted-imports': 'off',
      '@typescript-eslint/no-restricted-imports': restrict([RELATIVE], [RAW_CLIENT, NO_HAPTICS]),
      'no-restricted-globals': ['error', NO_FETCH],
      'no-restricted-properties': ['error', ...NO_FETCH_PROPERTY],
      'no-restricted-syntax': syntax(),
    },
  },
  {
    files: LITERAL_TOKEN_FILES,
    rules: { 'no-restricted-syntax': syntax({ literalTokens: true }) },
  },
  {
    files: SRC,
    rules: { 'max-lines': ['error', { max: FILE_MAX_LINES, skipBlankLines: true, skipComments: false }] },
  },
  {
    files: ['src/app/**/*.{ts,tsx}'],
    rules: {
      '@typescript-eslint/no-restricted-imports': restrict([RELATIVE, NO_APP], [RAW_CLIENT, NO_HAPTICS]),
      'max-lines': ['error', { max: ROUTE_MAX_LINES, skipBlankLines: true, skipComments: false }],
    },
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
  {
    files: ['src/lib/hooks/**/*.{ts,tsx}'],
    rules: {
      '@typescript-eslint/no-restricted-imports': restrictInHooks([RELATIVE, NO_APP, NO_COMPONENTS], [RAW_CLIENT, NO_HAPTICS]),
      'no-restricted-syntax': syntax({ effects: false }),
    },
  },
  {
    files: ['src/lib/hooks/**/*.{ts,tsx}', 'src/components/celebration/**/*.{ts,tsx}'],
    rules: { '@eslint-community/eslint-comments/no-restricted-disable': ['error', ...LOCKED_RULES] },
  },
]);
