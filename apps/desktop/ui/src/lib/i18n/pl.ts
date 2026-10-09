// Słownik polski — źródło kluczy i18n (en.ts musi mieć te same klucze; sprawdza to TypeScript).
// Liczby mnogie: { one, few, many, other } → Intl.PluralRules('pl-PL'). Agentki: rodzaj żeński.
import type { Message } from './core';
import { plAgents } from './pl-agents';
import { plApp } from './pl-app';
import { plBuilder } from './pl-builder';
import { plMemory } from './pl-memory';
import { plPlugins } from './pl-plugins';
import { plBundles } from './pl-bundles';
import { plModels } from './pl-models';
import { plSettings } from './pl-settings';
import { plHub } from './pl-hub';
import { plApi } from './pl-api';
import { plTasks } from './pl-tasks';
import { plUpdates } from './pl-updates';
import { plBroker } from './pl-broker';
import { plVoice } from './pl-voice';
import { plWork } from './pl-work';
import { plFiles } from './pl-files';
import { plCameras } from './pl-cameras';

export const pl = {
  ...plApp,
  ...plAgents,
  ...plSettings,
  ...plHub,
  ...plApi,
  ...plMemory,
  ...plTasks,
  ...plWork,
  ...plBuilder,
  ...plUpdates,
  ...plBroker,
  ...plVoice,
  ...plPlugins,
  ...plModels,
  ...plBundles,
  ...plFiles,
  ...plCameras,
} as const satisfies Record<string, Message>;

export type MessageKey = keyof typeof pl;
