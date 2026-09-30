import type { Meta, StoryObj } from '@storybook/svelte-vite';
import PanelDemo from './PanelDemo.svelte';

const meta = { title: 'Komponenty/Panel', component: PanelDemo } satisfies Meta<typeof PanelDemo>;
export default meta;

export const Karty: StoryObj<typeof meta> = {};
