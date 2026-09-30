// Słownik polski — źródło kluczy i18n (en.ts musi mieć te same klucze; sprawdza to TypeScript).
// Liczby mnogie: { one, few, many, other } → Intl.PluralRules('pl-PL'). Agentki: rodzaj żeński.
import type { Message } from './core';
import { plApp } from './pl-app';
import { plSettings } from './pl-settings';

export const pl = {
  ...plApp,
  ...plSettings,
} as const satisfies Record<string, Message>;

export type MessageKey = keyof typeof pl;
