// Słownik polski — źródło kluczy i18n (en.ts musi mieć te same klucze; sprawdza to TypeScript).
// Liczby mnogie: { one, few, many, other } → Intl.PluralRules('pl-PL'). Agentki: rodzaj żeński.
import type { Message } from './core';
import { plCore } from './pl-core';
import { plRest } from './pl-rest';

/** Cały słownik (źródło typu kluczy i testów); aplikacja ładuje go w dwóch częściach. */
export const pl = {
  ...plCore,
  ...plRest,
} as const satisfies Record<string, Message>;

export type MessageKey = keyof typeof pl;
