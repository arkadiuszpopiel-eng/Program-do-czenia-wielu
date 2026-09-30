import type { Meta, StoryObj } from '@storybook/svelte-vite';
import F1Gallery from './F1Gallery.svelte';

const meta = {
  title: 'Komponenty/F1 — galeria',
  component: F1Gallery,
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof F1Gallery>;
export default meta;

export const Jasny: StoryObj<typeof meta> = { globals: { theme: 'light' } };
export const Ciemny: StoryObj<typeof meta> = { globals: { theme: 'dark' } };
