// Słownik polski — źródło kluczy i18n (en.ts musi mieć te same klucze; sprawdza to TypeScript).
// Liczby mnogie: { one, few, many, other } → Intl.PluralRules('pl-PL'). Agentki: rodzaj żeński.
import type { Message } from './core';
import { plAgents } from './pl-agents';
import { plApp } from './pl-app';
import { plBuilder } from './pl-builder';
import { plMemory } from './pl-memory';
import { plSettings } from './pl-settings';
import { plTasks } from './pl-tasks';
import { plWork } from './pl-work';

export const pl = {
  ...plApp,
  ...plAgents,
  ...plSettings,
  ...plMemory,
  ...plTasks,
  ...plWork,
  ...plBuilder,
} as const satisfies Record<string, Message>;

export type MessageKey = keyof typeof pl;
