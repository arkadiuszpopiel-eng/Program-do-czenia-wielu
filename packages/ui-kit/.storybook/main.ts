import type { StorybookConfig } from '@storybook/svelte-vite';

const config: StorybookConfig = {
  framework: { name: '@storybook/svelte-vite', options: {} },
  stories: ['../src/**/*.stories.ts'],
  addons: ['@storybook/addon-a11y'],
  core: { disableTelemetry: true },
};

export default config;
