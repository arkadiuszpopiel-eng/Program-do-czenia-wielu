// Makieta 14: wprowadzenie — każdy krok osobno, jasny i ciemny, 1280 px.
import type { Meta, StoryObj } from '@storybook/svelte-vite';
import StoryApp from './StoryApp.svelte';

const meta = {
  title: 'Ekrany F1/14 Onboarding',
  component: StoryApp,
  parameters: { layout: 'fullscreen', a11y: { test: 'error' } },
  args: { scenario: 'first-run', view: 'onboarding' },
} satisfies Meta<typeof StoryApp>;
export default meta;

type Story = StoryObj<typeof meta>;
const step = (onboardingStep: number, theme: 'light' | 'dark'): Story => ({
  args: { onboardingStep, theme },
  globals: { theme },
});

export const K1_Mikrofon_jasny = step(0, 'light');
export const K1_Mikrofon_ciemny = step(0, 'dark');
export const K2_ProfilGlosu_jasny = step(1, 'light');
export const K2_ProfilGlosu_ciemny = step(1, 'dark');
export const K3_Sprzet_jasny = step(2, 'light');
export const K3_Sprzet_ciemny = step(2, 'dark');
export const K4_KontaIKlucze_jasny = step(3, 'light');
export const K4_KontaIKlucze_ciemny = step(3, 'dark');
export const K5_Autonomia_jasny = step(4, 'light');
export const K5_Autonomia_ciemny = step(4, 'dark');
export const K6_Korpus_jasny = step(5, 'light');
export const K6_Korpus_ciemny = step(5, 'dark');
export const K7_Import_jasny = step(6, 'light');
export const K7_Import_ciemny = step(6, 'dark');
