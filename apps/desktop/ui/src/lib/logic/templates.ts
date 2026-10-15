// Szablony promptów: zmienne `{{nazwa}}` (kolejność pierwszego wystąpienia, bez powtórzeń)
// i wypełnianie przed wstawieniem do pola wpisywania. Te same reguły co w rdzeniu
// (`app-modules/src/prompt_templates.rs`).

const VARIABLE = /\{\{\s*([\p{L}\p{N}_ -]{1,30}?)\s*\}\}/gu;

/** Zmienne szablonu. */
export function templateVariables(text: string): string[] {
  const out: string[] = [];
  for (const m of text.matchAll(VARIABLE)) {
    const name = m[1]?.trim();
    if (name && !out.includes(name)) out.push(name);
  }
  return out;
}

/** Tekst z podstawionymi zmiennymi; pusta wartość zostawia `{{nazwa}}` do uzupełnienia ręcznie. */
export function fillTemplate(text: string, values: Readonly<Record<string, string>>): string {
  return text.replace(VARIABLE, (whole, raw: string) => {
    const value = values[raw.trim()]?.trim();
    return value ? value : whole;
  });
}

/** Wstawienie do szkicu: pusty szkic — sam szablon, inaczej po pustej linii. */
export function appendToDraft(draft: string, text: string): string {
  return draft.trim() ? `${draft.trimEnd()}\n\n${text}` : text;
}

/** Najdłuższa nazwa i treść szablonu (jak w rdzeniu). */
export const TEMPLATE_NAME_MAX = 60;
export const TEMPLATE_TEXT_MAX = 8000;
export const TEMPLATE_COUNT_MAX = 100;

/**
 * Nazwa szablonu z treści wiadomości („Zapisz jako szablon”): pierwsza linia bez nadmiaru odstępów,
 * najwyżej 40 znaków z wielokropkiem; zajęta nazwa dostaje numer „(2)”, „(3)”…
 */
export function templateNameFrom(text: string, taken: readonly string[], fallback: string): string {
  const line = (text.trim().split('\n')[0] ?? '').replace(/\s+/g, ' ').trim();
  const base = line.length > 40 ? `${line.slice(0, 39).trimEnd()}…` : line || fallback;
  const used = new Set(taken.map((n) => n.toLowerCase()));
  if (!used.has(base.toLowerCase())) return base;
  for (let i = 2; ; i += 1) {
    const name = `${base} (${i})`;
    if (!used.has(name.toLowerCase())) return name;
  }
}
