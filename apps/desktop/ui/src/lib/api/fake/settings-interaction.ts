// Ustawienia Jarvisa odpowiadają katalogowi rdzenia (agents.interaction_mode i nadpisania person).
import type { SettingDef } from '../types-system';
import { L, select } from './settings-helpers';

const description = L(
  'Krótkie meldunki i zlecone zadania. Zachowuje tryb zgód i limity. Mikrofon włączasz osobno.',
  'Concise updates and requested tasks. Keeps approval mode and limits. Turn the microphone on separately.',
);
const options: [string, ReturnType<typeof L>][] = [
  ['standard', L('Standardowy', 'Standard')],
  ['jarvis', L('Jarvis — rozmowa i zadania', 'Jarvis — conversation and tasks')],
];

export const INTERACTION_SETTINGS: readonly SettingDef[] = [
  select(
    'agents.interaction_mode',
    L('Wspólny sposób współpracy', 'Common interaction style'),
    description,
    options,
    'standard',
  ),
  ...['alfa', 'beta', 'gama', 'delta'].map((id) =>
    select(
      `agents.${id}.interaction_mode`,
      L(`${id} — sposób współpracy`, `${id} — interaction style`),
      description,
      [['inherit', L('Jak ustawienie wspólne', 'Use common setting')], ...options],
      'inherit',
    ),
  ),
];
