// Słownik angielski (ładowany leniwie). Kompletność kluczy wymusza TypeScript: każdy klucz `pl`
// musi mieć odpowiednik (pliki en-app.ts / en-settings.ts są typowane kluczami z pl-*.ts).
import type { Message } from './core';
import { enApp } from './en-app';
import { enSettings } from './en-settings';
import type { MessageKey } from './pl';

export const en: Record<MessageKey, Message> = { ...enApp, ...enSettings };
