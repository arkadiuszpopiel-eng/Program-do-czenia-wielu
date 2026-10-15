// Interfejs szablonów promptów (część `AlfaClient`; komendy `prompt_templates_*`).
import type { PromptTemplate } from './types-templates';

export interface TemplatesApi {
  list(): Promise<readonly PromptTemplate[]>;
  /** Zapis całej listy (rdzeń sprawdza nazwy, treść i zmienne, nadaje identyfikatory). */
  save(templates: readonly PromptTemplate[]): Promise<readonly PromptTemplate[]>;
}
