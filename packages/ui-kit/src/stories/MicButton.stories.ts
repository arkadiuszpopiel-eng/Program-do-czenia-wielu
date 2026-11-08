import type { Meta, StoryObj } from '@storybook/svelte-vite';
import MicStates from './MicStates.svelte';

const meta = { title: 'Komponenty/MicButton', component: MicStates } satisfies Meta<
  typeof MicStates
>;
export default meta;

export const Stany: StoryObj<typeof meta> = {};
