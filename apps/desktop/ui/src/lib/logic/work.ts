// Czyste pomocniki F8: imię agentki (także spoza obsady standardowej), kodowanie strumienia
// terminala (base64 ↔ bajty, UTF-8), profil terminala dla mostu CLI, pusty szkic Kreatora
// i domyślne parametry umiejętności ze schematu JSON.
import { agents, type AgentId } from '@alfa/ui-kit';
import type { AgentDraft, TerminalProfileId } from '../api/types-work';

/** Imię agentki do wyświetlenia (agentki z Kreatora — identyfikator persony). */
export function agentName(id: string): string {
  return (agents as Readonly<Record<string, { name: string } | undefined>>)[id]?.name ?? id;
}

/** Czy identyfikator należy do obsady standardowej (awatar z ui-kit). */
export function isStandardAgent(id: string): id is AgentId {
  return Object.prototype.hasOwnProperty.call(agents, id);
}

/** Ramka `output` (base64) → bajty dla emulatora terminala. */
export function b64ToBytes(b64: string): Uint8Array {
  const raw = atob(b64);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
  return out;
}

function bytesToB64(bytes: Uint8Array): string {
  let raw = '';
  for (const b of bytes) raw += String.fromCharCode(b);
  return btoa(raw);
}

/** Wpisany tekst (UTF-16 z emulatora) → UTF-8 → base64 dla `terminal_input`. */
export function textToB64(text: string): string {
  return bytesToB64(new TextEncoder().encode(text));
}

/** Dane binarne emulatora (np. raporty myszy; znaki 0–255) → base64. */
export function binaryToB64(data: string): string {
  const bytes = new Uint8Array(data.length);
  for (let i = 0; i < data.length; i++) bytes[i] = data.charCodeAt(i) & 0xff;
  return bytesToB64(bytes);
}

/** Profil terminala logowania dla mostu CLI (`null` — most bez logowania w terminalu). */
export function loginProfile(bridge: string): TerminalProfileId | null {
  if (bridge === 'claude_code') return 'claude_login';
  if (bridge === 'codex') return 'codex_login';
  return null;
}

/** Pusty szkic agentki (formularz Kreatora). */
export function emptyDraft(): AgentDraft {
  return {
    id: null,
    name: null,
    forms: null,
    glyph: null,
    color: null,
    character: null,
    voice: { base: 'pl-f1', pitch: 1, rate: 1, perceived_age: 22, timbre: '', design_prompt: '' },
    role: {
      id: '',
      name: '',
      description: '',
      prompt: '',
      model_policy: 'conversation',
      tools: [],
      read_only: false,
      untrusted_isolated: false,
      author: false,
    },
    limits: {
      autonomy: 'L2',
      budget: null,
      fs_write: [],
      memory_scope: 'agent',
      retain_days: 30,
      triggers: [],
    },
    skills: [],
  };
}

/** Szablon parametrów ze schematu JSON umiejętności (wymagane pola tekstowe puste). */
export function paramsTemplate(schema: unknown): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  if (typeof schema !== 'object' || schema === null) return out;
  const { properties, required } = schema as {
    properties?: Record<string, { type?: string; default?: unknown }>;
    required?: string[];
  };
  for (const [key, def] of Object.entries(properties ?? {})) {
    if (def.default !== undefined) out[key] = def.default;
    else if ((required ?? []).includes(key))
      out[key] = def.type === 'number' || def.type === 'integer' ? 0 : '';
  }
  return out;
}
