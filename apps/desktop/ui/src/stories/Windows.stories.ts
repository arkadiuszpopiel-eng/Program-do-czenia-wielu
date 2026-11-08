// Makiety 4, 5 i 20: pigułka głosowa (podgląd w ramce), Szybkie pytanie, menu zasobnika.
import type { Meta, StoryObj } from '@storybook/svelte-vite';
import QuickStory from './QuickStory.svelte';
import TrayMenuPreview from './TrayMenuPreview.svelte';

const meta = {
  title: 'Ekrany F1/Okna dodatkowe',
  component: QuickStory,
  parameters: { layout: 'centered', a11y: { test: 'error' } },
} satisfies Meta<typeof QuickStory>;
export default meta;

type Story = StoryObj<typeof meta>;

/** 5 · Szybkie pytanie (640 px): odpowiedź rozwija się pod polem. */
export const S05_SzybkiePytanie_jasny: Story = {
  args: { theme: 'light', question: 'Ile dni ma październik?' },
  globals: { theme: 'light' },
};
export const S05_SzybkiePytanie_ciemny: Story = {
  args: { theme: 'dark', question: 'Ile dni ma październik?' },
  globals: { theme: 'dark' },
};

/** 20 · Menu zasobnika (natywne — tu podgląd układu i tekstów). */
export const S20_MenuZasobnika_jasny: StoryObj<typeof TrayMenuPreview> = {
  render: (args) => ({ Component: TrayMenuPreview, props: args }),
  args: { theme: 'light', listening: false },
  globals: { theme: 'light' },
};
export const S20_MenuZasobnika_ciemny: StoryObj<typeof TrayMenuPreview> = {
  render: (args) => ({ Component: TrayMenuPreview, props: args }),
  args: { theme: 'dark', listening: true },
  globals: { theme: 'dark' },
};
