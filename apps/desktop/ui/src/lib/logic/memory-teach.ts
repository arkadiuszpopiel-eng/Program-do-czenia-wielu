// „Naucz Alfę” (P5): podział treści na fakty tak jak w rdzeniu (`app-memory/src/chunk.rs`) —
// podgląd liczby faktów w oknie i atrapa; dozwolone zakresy (treść z zewnątrz tylko w sesji);
// cofnięcie nauki przez zapomnienie zapisanych faktów.
import type { RememberScope } from '../api/types';
import type { TeachSource } from '../api/types-memory';

export interface TeachLimits {
  readonly chunkChars: number;
  readonly maxInputChars: number;
  readonly maxFacts: number;
}

/** Limity rdzenia (`chunk::Limits::default`). */
export const TEACH_LIMITS: TeachLimits = { chunkChars: 800, maxInputChars: 100_000, maxFacts: 128 };

export interface TeachChunks {
  readonly pieces: readonly string[];
  readonly truncated: boolean;
  readonly duplicates: number;
}

const INVISIBLE = /[\u200B-\u200F\u202A-\u202E\u2066-\u2069\uFEFF]/gu;
const CONTROL = /\p{Cc}/gu;

const count = (s: string): number => Array.from(s).length;
const take = (s: string, n: number): string => Array.from(s).slice(0, n).join('');

/** Jedna linia: białe znaki zwinięte, bez znaków sterujących i niewidocznych. */
export function oneLine(text: string): string {
  return text
    .replace(/\s/gu, ' ')
    .replace(CONTROL, '')
    .replace(INVISIBLE, '')
    .split(' ')
    .filter(Boolean)
    .join(' ');
}

/** Akapity (oddzielone pustą linią), każdy jako jedna linia. */
export function paragraphs(text: string): string[] {
  return text
    .split(/\r?\n[ \t\f\v\r]*\r?\n/u)
    .map(oneLine)
    .filter(Boolean);
}

function pack(parts: readonly string[], max: number): string[] {
  const out: string[] = [];
  let current = '';
  for (const part of parts) {
    if (current && count(current) + 1 + count(part) > max) {
      out.push(current);
      current = '';
    }
    current = current ? `${current} ${part}` : part;
  }
  if (current) out.push(current);
  return out;
}

function byWords(sentence: string, max: number): string[] {
  const parts: string[] = [];
  for (const word of sentence.split(' ')) {
    if (count(word) <= max) parts.push(word);
    else {
      const chars = Array.from(word);
      for (let i = 0; i < chars.length; i += max) parts.push(chars.slice(i, i + max).join(''));
    }
  }
  return pack(parts, max);
}

function splitLine(line: string, max: number): string[] {
  if (count(line) <= max) return [line];
  return line
    .split(/(?<=[.!?…;]) /u)
    .filter(Boolean)
    .flatMap((s) => (count(s) <= max ? [s] : byWords(s, max)));
}

/** Zapas znaków ponad limit wejścia przy wstępnym ucięciu surowego tekstu (jak rdzeń). */
const RAW_MARGIN_CHARS = 20_000;

/**
 * Wstępne ucięcie surowego tekstu: podział nie przechodzi całego, bardzo długiego wejścia (podgląd
 * liczony przy każdym naciśnięciu klawisza), skoro i tak liczy się tylko początek. Jednostki
 * UTF-16 ×2 — co najmniej tyle znaków, ile bierze rdzeń.
 */
export function capInput(text: string, limits: TeachLimits = TEACH_LIMITS): string {
  const units = 2 * (limits.maxInputChars + RAW_MARGIN_CHARS);
  return text.length > units ? text.slice(0, units) : text;
}

/** Fragmenty-fakty tekstu (akapity → zdania → słowa), z limitami i bez powtórzeń. */
export function chunkText(text: string, limits: TeachLimits = TEACH_LIMITS): TeachChunks {
  const max = Math.max(1, limits.chunkChars);
  let budget = limits.maxInputChars;
  const raw = capInput(text, limits);
  let truncated = raw.length < text.length;
  const parts: string[] = [];
  for (const paragraph of paragraphs(raw)) {
    if (budget <= 0) {
      truncated = true;
      break;
    }
    let line = paragraph;
    if (count(line) > budget) {
      truncated = true;
      line = take(line, budget).trimEnd();
    }
    budget -= count(line);
    parts.push(...splitLine(line, max));
  }
  const seen = new Set<string>();
  const pieces: string[] = [];
  let duplicates = 0;
  for (const piece of pack(parts, max)) {
    const key = piece.toLocaleLowerCase('pl');
    if (seen.has(key)) duplicates++;
    else if (pieces.length < limits.maxFacts) {
      seen.add(key);
      pieces.push(piece);
    } else truncated = true;
  }
  return { pieces, truncated, duplicates };
}

/** Treść z zewnątrz (tekst, plik) — niezaufana: dane, nie polecenia. */
export function isExternal(kind: TeachSource['kind']): boolean {
  return kind !== 'conversation';
}

/** Zakresy dozwolone dla źródła (rdzeń odmawia innych). */
export function teachScopes(kind: TeachSource['kind']): readonly RememberScope[] {
  return isExternal(kind) ? ['session'] : ['session', 'project', 'global'];
}

/**
 * Cofnięcie nauki: zapomina zapisane fakty po kolei; zwraca liczbę usuniętych, nieudanych
 * i identyfikatory nieudanych (do ponowienia).
 */
export async function undoTeach(
  ids: readonly string[],
  forget: (id: string) => Promise<unknown>,
): Promise<{ removed: number; failed: number; left: string[] }> {
  let removed = 0;
  const left: string[] = [];
  for (const id of ids) {
    try {
      await forget(id);
      removed++;
    } catch {
      left.push(id);
    }
  }
  return { removed, failed: left.length, left };
}

/** Rodzaj zakresu z klucza rdzenia (`session:<id>`, `project:<id>`, `agent:<id>`, `global`). */
export function scopeOfKey(key: string): RememberScope {
  const kind = key.split(':')[0];
  return kind === 'project' || kind === 'global' || kind === 'agent' ? kind : 'session';
}
