import type { Meta, StoryObj } from '@storybook/svelte-vite';
import TokensGallery from './TokensGallery.svelte';

const meta = { title: 'Fundamenty/Tokeny', component: TokensGallery } satisfies Meta<
  typeof TokensGallery
>;
export default meta;

export const Tokeny: StoryObj<typeof meta> = {};
