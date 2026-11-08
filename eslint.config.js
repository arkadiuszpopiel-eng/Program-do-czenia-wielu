// Konfiguracja ESLint 9 (flat). Egzekwuje Svelte 5 z runes: zakaz `export let`,
// zakaz `$:` oraz dyrektyw `on:` (AGENTS.md → „Stos"). Reguły techniczne, nie styl —
// styl pilnuje Prettier.
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';

/** Składnia Svelte 4 zabroniona w plikach .svelte (selektory AST svelte-eslint-parser). */
const svelte4Forbidden = [
  {
    selector: 'ExportNamedDeclaration > VariableDeclaration[kind="let"]',
    message: 'Svelte 4: `export let` — użyj `let { … } = $props()`.',
  },
  {
    selector: 'SvelteReactiveStatement, LabeledStatement[label.name="$"]',
    message: 'Svelte 4: `$:` — użyj `$derived()` / `$effect()`.',
  },
  {
    selector: 'SvelteDirective[kind="EventHandler"]',
    message: 'Svelte 4: `on:event` — użyj atrybutu `onevent`.',
  },
  {
    selector: 'CallExpression[callee.name="createEventDispatcher"]',
    message: 'Svelte 4: `createEventDispatcher` — przekaż callback przez `$props()`.',
  },
];

export default ts.config(
  {
    ignores: [
      '**/node_modules/**',
      '**/dist/**',
      '**/storybook-static/**',
      '**/target/**',
      'packages/ui-kit/src/tokens.ts',
      'docs/**',
      'crates/**',
      'providers-catalog/**',
      '.github/**',
    ],
  },
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs.recommended,
  {
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
    },
    rules: {
      'no-restricted-syntax': ['error', ...svelte4Forbidden],
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrorsIgnorePattern: '^_' },
      ],
    },
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
    languageOptions: {
      parserOptions: {
        parser: ts.parser,
        extraFileExtensions: ['.svelte'],
      },
    },
    rules: {
      'svelte/valid-compile': 'error',
      'svelte/no-at-html-tags': 'error',
      'svelte/no-svelte-internal': 'error',
      'svelte/require-each-key': 'error',
      'svelte/no-unused-svelte-ignore': 'error',
      'svelte/prefer-svelte-reactivity': 'error',
      'svelte/no-reactive-reassign': 'error',
    },
  },
  {
    // `{' '}` przed separatorem „·” jest celowe: bez niego Svelte skleja kropkę z poprzednim
    // słowem (pilnuje tego scripts/check-svelte5.mjs). Reguła zostaje włączona wszędzie indziej;
    // nowy plik z takim separatorem trzeba tu dopisać świadomie.
    files: [
      'apps/desktop/ui/src/lib/components/panels/MemoryDetail.svelte',
      'apps/desktop/ui/src/lib/components/panels/MemoryPanel.svelte',
      'apps/desktop/ui/src/lib/components/panels/TaskNode.svelte',
      'apps/desktop/ui/src/lib/components/panels/TimelinePanel.svelte',
      'apps/desktop/ui/src/lib/views/settings/pages/MemoryPage.svelte',
      'apps/desktop/ui/src/lib/views/settings/pages/TransferImport.svelte',
      'apps/desktop/ui/src/lib/views/settings/pages/TriggersPage.svelte',
    ],
    rules: { 'svelte/no-useless-mustaches': 'off' },
  },
);
