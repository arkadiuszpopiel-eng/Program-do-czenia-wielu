import type { Meta, StoryObj } from '@storybook/svelte-vite';
import ButtonGallery from './ButtonGallery.svelte';

const meta = { title: 'Komponenty/Button + IconButton', component: ButtonGallery } satisfies Meta<
  typeof ButtonGallery
>;
export default meta;

export const Warianty: StoryObj<typeof meta> = {};
