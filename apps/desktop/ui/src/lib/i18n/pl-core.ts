// Słownik polski potrzebny od startu okna głównego (pasek, rozmowa, panele startowe).
// Działy okien otwieranych później są w `pl-rest.ts` (ładowany zaraz po starcie, osobno).
import type { Message } from './core';
import { plApp } from './pl-app';
import { plAgents } from './pl-agents';
import { plSettings } from './pl-settings';
import { plTasks } from './pl-tasks';
import { plWork } from './pl-work';
import { plUpdates } from './pl-updates';
import { plBroker } from './pl-broker';
import { plFiles } from './pl-files';
import { plCameras } from './pl-cameras';
import { plTemplates } from './pl-templates';
import { plHelp } from './pl-help';
import { plBoard } from './pl-board';
import { plStats } from './pl-stats';
import { plArchive } from './pl-archive';
import { plCanvas } from './pl-canvas';
import { plArena } from './pl-arena';
import { plTeach } from './pl-teach';

export const plCore = {
  ...plApp,
  ...plAgents,
  ...plSettings,
  ...plTasks,
  ...plWork,
  ...plUpdates,
  ...plBroker,
  ...plFiles,
  ...plCameras,
  ...plTemplates,
  ...plHelp,
  ...plBoard,
  ...plStats,
  ...plArchive,
  ...plCanvas,
  ...plArena,
  ...plTeach,
} as const satisfies Record<string, Message>;
