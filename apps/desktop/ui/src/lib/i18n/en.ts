// Słownik angielski (ładowany leniwie). Kompletność kluczy wymusza TypeScript: każdy klucz `pl`
// musi mieć odpowiednik (pliki en-app.ts / en-settings.ts są typowane kluczami z pl-*.ts).
import type { Message } from './core';
import { enAgents } from './en-agents';
import { enApp } from './en-app';
import { enBuilder } from './en-builder';
import { enMemory } from './en-memory';
import { enPlugins } from './en-plugins';
import { enBundles } from './en-bundles';
import { enModels } from './en-models';
import { enSettings } from './en-settings';
import { enHub } from './en-hub';
import { enApi } from './en-api';
import { enTasks } from './en-tasks';
import { enUpdates } from './en-updates';
import { enBroker } from './en-broker';
import { enVoice } from './en-voice';
import { enWork } from './en-work';
import { enFiles } from './en-files';
import { enCameras } from './en-cameras';
import { enRouting } from './en-routing';
import { enConnections } from './en-connections';
import { enMcp } from './en-mcp';
import { enDiag } from './en-diag';
import { enModules } from './en-modules';
import { enAdvanced } from './en-advanced';
import { enCommands } from './en-commands';
import { enTemplates } from './en-templates';
import { enRoutes } from './en-routes';
import { enHelp } from './en-help';
import { enBoard } from './en-board';
import { enJson } from './en-json';
import { enStats } from './en-stats';
import { enArchive } from './en-archive';
import type { MessageKey } from './pl';

export const en: Record<MessageKey, Message> = {
  ...enApp,
  ...enAgents,
  ...enSettings,
  ...enHub,
  ...enApi,
  ...enMemory,
  ...enTasks,
  ...enWork,
  ...enBuilder,
  ...enUpdates,
  ...enBroker,
  ...enVoice,
  ...enPlugins,
  ...enModels,
  ...enBundles,
  ...enFiles,
  ...enCameras,
  ...enRouting,
  ...enConnections,
  ...enMcp,
  ...enDiag,
  ...enModules,
  ...enAdvanced,
  ...enCommands,
  ...enTemplates,
  ...enRoutes,
  ...enHelp,
  ...enBoard,
  ...enJson,
  ...enStats,
  ...enArchive,
};
