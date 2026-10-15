// Szablony promptów (komendy `prompt_templates_*`): tekst ze zmiennymi `{{nazwa}}`.

export interface PromptTemplate {
  /** Pusty przy dodawaniu — nadaje rdzeń. */
  readonly id: string;
  readonly name: string;
  /** Treść ze zmiennymi `{{nazwa}}`. */
  readonly text: string;
}
