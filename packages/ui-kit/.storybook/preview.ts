import type { Preview } from '@storybook/svelte-vite';
import '../src/tokens.css';
import '../src/base.css';

/** Przełącznik motywu: ustawia data-theme na <html>, tak jak zrobi to aplikacja. */
const withTheme = (theme: string) => {
  const root = document.documentElement;
  if (theme === 'auto') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', theme);
  root.style.background = 'var(--alfa-color-bg)';
};

const preview: Preview = {
  globalTypes: {
    theme: {
      description: 'Motyw kolorystyczny',
      toolbar: {
        title: 'Motyw',
        icon: 'paintbrush',
        items: [
          { value: 'auto', title: 'Auto (system)' },
          { value: 'light', title: 'Jasny' },
          { value: 'dark', title: 'Ciemny' },
        ],
        dynamicTitle: true,
      },
    },
  },
  initialGlobals: { theme: 'auto' },
  decorators: [
    (story, context) => {
      withTheme(String(context.globals['theme'] ?? 'auto'));
      return story();
    },
  ],
  parameters: {
    layout: 'centered',
    backgrounds: { disable: true },
    a11y: { test: 'error' },
    controls: { matchers: { color: /(background|color)$/i } },
  },
};

export default preview;
