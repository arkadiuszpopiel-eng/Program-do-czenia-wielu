// Kopiowanie i wklejanie list jako JSON (szablony promptów, własne polecenia): eksport bez
// identyfikatorów (nadaje je rdzeń przy zapisie) i import ze sprawdzeniem kształtu. Ostateczną
// walidację (nazwy, długości, wyrażenia) robi rdzeń przy zapisie całej listy.
import type { CustomCommand, QuickMatchMode } from '../api/types-quick';
import type { PromptTemplate } from '../api/types-templates';

/** Najwięcej pozycji w jednym wklejeniu. */
export const MAX_IMPORT = 100;

export type ImportError =
  | { readonly kind: 'json' }
  | { readonly kind: 'empty' }
  | { readonly kind: 'tooMany'; readonly max: number }
  | { readonly kind: 'shape'; readonly index: number; readonly fields: string };

export type ImportResult<T> =
  { readonly ok: true; readonly items: T[] } | { readonly ok: false; readonly error: ImportError };

/** Lista bez pola `id` — czytelna i bez kolizji przy wklejeniu w innej instalacji. */
export function exportJson<T extends { readonly id: string }>(items: readonly T[]): string {
  return JSON.stringify(
    items.map(({ id: _id, ...rest }) => rest),
    null,
    2,
  );
}

function records(text: string): ImportResult<Record<string, unknown>> {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return { ok: false, error: { kind: 'json' } };
  }
  const list = Array.isArray(value) ? value : [value];
  if (!list.length) return { ok: false, error: { kind: 'empty' } };
  if (list.length > MAX_IMPORT) return { ok: false, error: { kind: 'tooMany', max: MAX_IMPORT } };
  const items: Record<string, unknown>[] = [];
  for (const [i, item] of list.entries()) {
    if (typeof item !== 'object' || item === null || Array.isArray(item)) {
      return { ok: false, error: { kind: 'shape', index: i + 1, fields: 'obiekt' } };
    }
    items.push(item as Record<string, unknown>);
  }
  return { ok: true, items };
}

const str = (v: unknown): v is string => typeof v === 'string' && v.trim() !== '';

/** Szablony: `name` i `text` (napisy). */
export function parseTemplates(text: string): ImportResult<PromptTemplate> {
  const parsed = records(text);
  if (!parsed.ok) return parsed;
  const items: PromptTemplate[] = [];
  for (const [i, r] of parsed.items.entries()) {
    if (!str(r.name) || !str(r.text)) {
      return { ok: false, error: { kind: 'shape', index: i + 1, fields: 'name, text' } };
    }
    items.push({ id: '', name: r.name, text: r.text });
  }
  return { ok: true, items };
}

const MODES: readonly QuickMatchMode[] = ['exact', 'starts', 'contains', 'regex'];

/** Polecenia: `name`, `phrases` (lista napisów), `reply`; `mode`, `enabled`, `priority` opcjonalne. */
export function parseCommands(text: string): ImportResult<CustomCommand> {
  const parsed = records(text);
  if (!parsed.ok) return parsed;
  const items: CustomCommand[] = [];
  for (const [i, r] of parsed.items.entries()) {
    const phrases = Array.isArray(r.phrases) ? r.phrases.filter(str) : [];
    const mode = MODES.find((m) => m === r.mode) ?? (r.mode === undefined ? 'exact' : null);
    if (!str(r.name) || !str(r.reply) || !phrases.length || !mode) {
      return {
        ok: false,
        error: { kind: 'shape', index: i + 1, fields: 'name, phrases, reply, mode' },
      };
    }
    const priority = typeof r.priority === 'number' && Number.isFinite(r.priority) ? r.priority : 0;
    items.push({
      id: '',
      name: r.name,
      phrases,
      mode,
      reply: r.reply,
      enabled: r.enabled !== false,
      priority: Math.max(-10, Math.min(10, Math.round(priority))),
    });
  }
  return { ok: true, items };
}
