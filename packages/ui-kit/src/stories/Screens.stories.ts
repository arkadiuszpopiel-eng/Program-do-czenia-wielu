import type { Meta, StoryObj } from '@storybook/svelte-vite';
import StartScreen from '../screens/StartScreen.svelte';
import ConversationScreen from '../screens/ConversationScreen.svelte';
import VoiceModeScreen from '../screens/VoiceModeScreen.svelte';
import { mockCaptions, mockMessages, mockSessions, startSuggestions } from '../mock';

const meta = {
  title: 'Screens',
  component: StartScreen,
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof StartScreen>;
export default meta;

/** Makieta 1: Start / pusty stan z 3 sugestiami. */
export const Start: StoryObj<typeof meta> = {
  args: { suggestions: startSuggestions, micState: 'off' },
};

/** Makieta 2: Rozmowa (układ §14.2: pasek tytułu, Sesje, kolumna z wiadomościami i kapsułą, panel prawy, composer). */
export const Conversation: StoryObj<typeof ConversationScreen> = {
  render: (args) => ({ Component: ConversationScreen, props: args }),
  args: {
    project: 'Projekt X',
    session: 'Raport Q3',
    sessions: mockSessions,
    messages: mockMessages,
    activity: {
      agent: 'delta',
      description: 'edytuję raport.docx',
      step: 3,
      totalSteps: 7,
      elapsedSeconds: 42,
    },
    micState: 'listening',
    leftOpen: true,
    rightOpen: true,
    toast: { message: 'Delta: przeniesiono 14 plików · 2,1 s', actionLabel: 'Cofnij' },
  },
};

export const ConversationCompact: StoryObj<typeof ConversationScreen> = {
  ...Conversation,
  name: 'Conversation — tylko rozmowa',
  args: { ...Conversation.args, leftOpen: false, rightOpen: false, toast: undefined },
};

/** Makieta 3: Pełny tryb głosowy (orb Canvas 2D, napisy na żywo, stany mikrofonu). */
export const VoiceMode: StoryObj<typeof VoiceModeScreen> = {
  render: (args) => ({ Component: VoiceModeScreen, props: args }),
  args: { agent: 'beta', micState: 'speaking', lines: mockCaptions, simulate: true },
  argTypes: {
    micState: {
      control: 'select',
      options: ['off', 'listening', 'hearing', 'processing', 'speaking', 'muted', 'dnd'],
    },
    agent: { control: 'select', options: ['alfa', 'beta', 'gama', 'delta'] },
  },
};
