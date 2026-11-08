import type { Meta, StoryObj } from '@storybook/svelte-vite';
import ChipGallery from './ChipGallery.svelte';

const meta = { title: 'Komponenty/Chip + Avatar', component: ChipGallery } satisfies Meta<
  typeof ChipGallery
>;
export default meta;

export const Warianty: StoryObj<typeof meta> = {};
