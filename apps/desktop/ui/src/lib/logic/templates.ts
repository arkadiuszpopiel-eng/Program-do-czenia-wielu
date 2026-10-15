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
