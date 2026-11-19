import type { KnipConfig } from 'knip';

const RUN_BY_THE_ROOT_CHECK_SCRIPT = ['tsx'];

const config: KnipConfig = {
  entry: ['src/app/**/*.{ts,tsx}', 'src/widget/**/*.{ts,tsx}', 'scripts/*.mjs', 'src/**/*.test.ts'],
  project: ['src/**/*.{ts,tsx}', 'scripts/*.mjs'],
  ignoreDependencies: RUN_BY_THE_ROOT_CHECK_SCRIPT,
};

export default config;
