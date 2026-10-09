import { defineConfig } from 'eslint/config'
import js from '@eslint/js'
import globals from 'globals'
import tseslint from 'typescript-eslint'
import eslintPluginSvelte from 'eslint-plugin-svelte'
import eslintPluginPrettierRecommended from 'eslint-plugin-prettier/recommended'

// Mirrors what @electron-toolkit/eslint-config-ts's `configs.recommended` composed (base
// JS/globals config + typescript-eslint recommended + a handful of extra TS rules), inlined
// directly now that the app no longer depends on that Electron-branded wrapper package.
const recommended = [
  js.configs.recommended,
  {
    languageOptions: {
      ecmaVersion: 2022,
      globals: { ...globals.browser, ...globals.es2021, ...globals.node },
      parserOptions: { ecmaFeatures: { jsx: true }, ecmaVersion: 2022, sourceType: 'module' },
      sourceType: 'module'
    }
  },
  ...tseslint.configs.recommended,
  {
    rules: {
      '@typescript-eslint/ban-ts-comment': ['error', { 'ts-ignore': 'allow-with-description' }],
      '@typescript-eslint/explicit-function-return-type': [
        'error',
        {
          allowExpressions: true,
          allowTypedFunctionExpressions: true,
          allowHigherOrderFunctions: true,
          allowIIFEs: true
        }
      ],
      '@typescript-eslint/explicit-module-boundary-types': 'off',
      '@typescript-eslint/no-empty-function': ['error', { allow: ['arrowFunctions'] }],
      '@typescript-eslint/no-empty-object-type': ['error', { allowInterfaces: 'always' }],
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-non-null-assertion': 'off',
      '@typescript-eslint/no-require-imports': 'error',
      '@typescript-eslint/no-unused-expressions': [
        'error',
        { allowShortCircuit: true, allowTaggedTemplates: true, allowTernary: true }
      ]
    }
  },
  {
    files: ['*.js', '*.mjs'],
    rules: { '@typescript-eslint/explicit-function-return-type': 'off' }
  }
]

// Mirrors @electron-toolkit/eslint-config-prettier: eslint-plugin-prettier's recommended
// preset with the prettier rule downgraded from 'error' to 'warn'.
const prettierRecommended = {
  ...eslintPluginPrettierRecommended,
  rules: { ...eslintPluginPrettierRecommended.rules, 'prettier/prettier': 'warn' }
}

export default defineConfig(
  { ignores: ['**/node_modules', '**/dist', '**/out'] },
  recommended,
  eslintPluginSvelte.configs['flat/recommended'],
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser
      }
    }
  },
  {
    files: ['**/*.{tsx,svelte}'],
    rules: {
      'svelte/no-unused-svelte-ignore': 'off'
    }
  },
  {
    files: ['**/*.svelte.ts'],
    languageOptions: {
      parser: tseslint.parser
    }
  },
  prettierRecommended
)
