import type { Meta, StoryObj } from '@storybook/svelte-vite';
import ActivityCapsule from '../components/ActivityCapsule.svelte';
import ApprovalCard from '../components/ApprovalCard.svelte';
import Toast from '../components/Toast.svelte';

const meta = {
  title: 'Komponenty/ActivityCapsule',
  component: ActivityCapsule,
  args: {
    agent: 'delta',
    description: 'edytuję raport.docx',
    step: 3,
    totalSteps: 7,
    elapsedSeconds: 42,
    onstop: () => {},
  },
  argTypes: { agent: { control: 'select', options: ['alfa', 'beta', 'gama', 'delta'] } },
} satisfies Meta<typeof ActivityCapsule>;
export default meta;

export const Kapsula: StoryObj<typeof meta> = { name: 'ActivityCapsule' };

export const Zatwierdzenie: StoryObj<typeof ApprovalCard> = {
  name: 'ApprovalCard',
  render: (args) => ({ Component: ApprovalCard, props: args }),
  args: {
    agent: 'delta',
    what: 'Zapis pliku Raporty/raport-Q3.docx i nadpisanie szablonu zarząd.dotx',
    why: 'Zarząd używa tego szablonu; potrzebuję zaktualizować stopkę z kwartałem.',
    reversible: true,
    risk: 'medium',
  },
};

export const ZatwierdzenieWysokie: StoryObj<typeof ApprovalCard> = {
  name: 'ApprovalCard — wysokie ryzyko',
  render: (args) => ({ Component: ApprovalCard, props: args }),
  args: {
    agent: 'gama',
    what: 'Usunięcie 214 plików w Pobrane/ starszych niż 90 dni',
    why: 'Zwolnienie 3,2 GB zgodnie z Twoją prośbą o porządki.',
    reversible: false,
    risk: 'high',
  },
};

export const Powiadomienie: StoryObj<typeof Toast> = {
  name: 'Toast',
  render: (args) => ({ Component: Toast, props: args }),
  args: {
    kind: 'success',
    message: 'Delta: przeniesiono 14 plików · 2,1 s',
    actionLabel: 'Cofnij',
    onaction: () => {},
    onclose: () => {},
  },
  argTypes: { kind: { control: 'select', options: ['info', 'success', 'warning', 'error'] } },
};
