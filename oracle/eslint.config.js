import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import prettierConfig from 'eslint-config-prettier';

export default [
  js.configs.recommended,
  ...tseslint.configs.recommendedTypeChecked,
  prettierConfig,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-unused-vars': 'error',
      'no-console': 'error',
      '@typescript-eslint/dot-notation': ['error', { allowIndexSignaturePropertyAccess: false }],
      'no-restricted-properties': [
        'error',
        {
          object: 'process',
          property: 'env',
          message: 'Read environment variables through src/config.ts.',
        },
      ],
      // Catches floating promises like the one fixed in #1036.
      '@typescript-eslint/no-floating-promises': 'error',
    },
  },
  {
    files: ['src/config.ts', 'src/**/*.test.ts'],
    rules: {
      'no-restricted-properties': 'off',
    },
  },
];
