import babelParser from '@babel/eslint-parser';
import sonarjs from 'eslint-plugin-sonarjs';

export default [
  { ignores: ['dist/**'] },
  {
    files: ['**/*.{ts,mjs}'],
    plugins: { sonarjs },
    // Keep these checks aligned with the Sonar rules validated locally.
    rules: {
      'sonarjs/cognitive-complexity': 'error',
      'sonarjs/no-nested-conditional': 'error',
      'sonarjs/no-nested-template-literals': 'error',
      'sonarjs/no-unenclosed-multiline-block': 'error',
    },
  },
  {
    files: ['**/*.ts'],
    languageOptions: {
      // Parse syntax without depending on the legacy TypeScript compiler API.
      parser: babelParser,
      parserOptions: {
        requireConfigFile: false,
        babelOptions: {
          babelrc: false,
          configFile: false,
          parserOpts: { plugins: ['typescript'] },
        },
      },
    },
  },
];
