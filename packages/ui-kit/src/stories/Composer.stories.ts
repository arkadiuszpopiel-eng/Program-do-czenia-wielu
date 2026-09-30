import type { Meta, StoryObj } from '@storybook/svelte-vite';
import ComposerDemo from './ComposerDemo.svelte';

const meta = { title: 'Komponenty/Composer', component: ComposerDemo } satisfies Meta<
  typeof ComposerDemo
>;
export default meta;

export const Domyslny: StoryObj<typeof meta> = {
  args: { micState: 'listening', sendOnEnter: true },
};
export const CtrlEnterWysyla: StoryObj<typeof meta> = {
  name: 'Ctrl+Enter wysyła (Enter = nowa linia)',
  args: { micState: 'off', sendOnEnter: false },
};
