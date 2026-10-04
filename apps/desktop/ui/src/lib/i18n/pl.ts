// Słownik polski — źródło kluczy i18n (en.ts musi mieć te same klucze; sprawdza to TypeScript).
// Liczby mnogie: { one, few, many, other } → Intl.PluralRules('pl-PL'). Agentki: rodzaj żeński.
import type { Message } from './core';
import { plAgents } from './pl-agents';
import { plApp } from './pl-app';
import { plBuilder } from './pl-builder';
import { plMemory } from './pl-memory';
import { plPlugins } from './pl-plugins';
import { plModels } from './pl-models';
import { plSettings } from './pl-settings';
import { plTasks } from './pl-tasks';
import { plUpdates } from './pl-updates';
import { plBroker } from './pl-broker';
import { plVoice } from './pl-voice';
import { plWork } from './pl-work';

export const pl = {
  ...plApp,
  ...plAgents,
  ...plSettings,
  ...plMemory,
  ...plTasks,
  ...plWork,
  ...plBuilder,
  ...plUpdates,
  ...plBroker,
  ...plVoice,
  ...plPlugins,
  ...plModels,
} as const satisfies Record<string, Message>;

export type MessageKey = keyof typeof pl;
