// Grupy stron w nawigacji Ustawień: 29 stron na jednej płaskiej liście było trudne do przejrzenia.
// Kolejność grup i stron w grupie jest stała; strona spoza listy (np. dodana przez moduł) trafia do
// grupy „other”, żeby nigdy nie zniknęła z nawigacji.

export const SETTINGS_GROUPS = [
  { id: 'basics', pages: ['general', 'appearance', 'language', 'shortcuts', 'notifications'] },
  { id: 'models', pages: ['providers', 'models', 'costs', 'router'] },
  { id: 'voice', pages: ['voice', 'devices'] },
  { id: 'agents', pages: ['agents', 'builder', 'skills', 'plugins', 'triggers', 'marshal'] },
  { id: 'safety', pages: ['permissions', 'computer'] },
  { id: 'data', pages: ['memory', 'sessions', 'files', 'transfer', 'logs'] },
  { id: 'system', pages: ['improve', 'modules', 'updates', 'advanced', 'about'] },
] as const;

export type SettingsGroupId = (typeof SETTINGS_GROUPS)[number]['id'] | 'other';

export interface SettingsGroup<P> {
  readonly id: SettingsGroupId;
  readonly pages: readonly P[];
}

/** Dzieli strony ze schematu na grupy; puste grupy pomija, nieznane strony zbiera w „other”. */
export function groupSettingsPages<P extends { readonly id: string }>(
  pages: readonly P[],
): SettingsGroup<P>[] {
  const byId = new Map(pages.map((p) => [p.id, p]));
  const used = new Set<string>();
  const out: SettingsGroup<P>[] = [];
  for (const group of SETTINGS_GROUPS) {
    const found: P[] = [];
    for (const id of group.pages) {
      const page = byId.get(id);
      if (page) {
        found.push(page);
        used.add(id);
      }
    }
    if (found.length) out.push({ id: group.id, pages: found });
  }
  const rest = pages.filter((p) => !used.has(p.id));
  if (rest.length) out.push({ id: 'other', pages: rest });
  return out;
}
