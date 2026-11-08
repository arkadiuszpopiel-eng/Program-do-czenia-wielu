import type { Meta, StoryObj } from '@storybook/svelte-vite';
import PaletteDemo from './PaletteDemo.svelte';

const meta = { title: 'Komponenty/CommandPalette', component: PaletteDemo } satisfies Meta<
  typeof PaletteDemo
>;
export default meta;

export const Paleta: StoryObj<typeof meta> = {};
