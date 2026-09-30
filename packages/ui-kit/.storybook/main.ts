import type { StorybookConfig } from '@storybook/svelte-vite';
import { fileURLToPath } from 'node:url';

// Stories ui-kit (komponenty, tokeny, makiety F0) + makiety ekranów F1 z aplikacji
// (`apps/desktop/ui/src/stories`) — renderowane na prawdziwych komponentach z atrapą backendu.
const appSrc = fileURLToPath(new URL('../../../apps/desktop/ui/src', import.meta.url));

const config: StorybookConfig = {
  framework: { name: '@storybook/svelte-vite', options: {} },
  stories: ['../src/**/*.stories.ts', '../../../apps/desktop/ui/src/stories/*.stories.ts'],
  addons: ['@storybook/addon-a11y'],
  core: { disableTelemetry: true },
  viteFinal: async (config) => {
    config.resolve ??= {};
    config.resolve.alias = { ...(config.resolve.alias ?? {}), $lib: `${appSrc}/lib` };
    config.worker = { format: 'es' };
    return config;
  },
};

export default config;
