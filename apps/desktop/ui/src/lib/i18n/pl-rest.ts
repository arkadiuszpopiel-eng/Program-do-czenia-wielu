// Słownik polski okien otwieranych później (Hub, modele, głos, wtyczki, MCP, diagnostyka…):
// poza paczką startową, ładowany równolegle ze startem (`restReady` w `i18n.svelte.ts`).
// Kod paczki startowej nie może używać tych kluczy — pilnuje tego test `dictionaries.test.ts`.
import type { Message } from './core';
import { plHub } from './pl-hub';
import { plApi } from './pl-api';
import { plMemory } from './pl-memory';
import { plBuilder } from './pl-builder';
import { plVoice } from './pl-voice';
import { plPlugins } from './pl-plugins';
import { plModels } from './pl-models';
import { plBundles } from './pl-bundles';
import { plRouting } from './pl-routing';
import { plConnections } from './pl-connections';
import { plMcp } from './pl-mcp';
import { plDiag } from './pl-diag';
import { plModules } from './pl-modules';
import { plAdvanced } from './pl-advanced';
import { plCommands } from './pl-commands';
import { plRoutes } from './pl-routes';
import { plJson } from './pl-json';

export const plRest = {
  ...plHub,
  ...plApi,
  ...plMemory,
  ...plBuilder,
  ...plVoice,
  ...plPlugins,
  ...plModels,
  ...plBundles,
  ...plRouting,
  ...plConnections,
  ...plMcp,
  ...plDiag,
  ...plModules,
  ...plAdvanced,
  ...plCommands,
  ...plRoutes,
  ...plJson,
} as const satisfies Record<string, Message>;
