// Słownik angielski (ładowany leniwie). Kompletność kluczy wymusza TypeScript: każdy klucz `pl`
// musi mieć odpowiednik (pliki en-app.ts / en-settings.ts są typowane kluczami z pl-*.ts).
import type { Message } from './core';
import { enAgents } from './en-agents';
import { enApp } from './en-app';
import { enMemory } from './en-memory';
import { enSettings } from './en-settings';
import { enTasks } from './en-tasks';
import type { MessageKey } from './pl';

export const en: Record<MessageKey, Message> = {
  ...enApp,
  ...enAgents,
  ...enSettings,
  ...enMemory,
  ...enTasks,
};
