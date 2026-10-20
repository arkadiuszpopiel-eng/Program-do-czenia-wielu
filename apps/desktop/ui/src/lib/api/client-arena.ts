// Metody Areny (P4) w `TurnsApi`: komendy `turns_arena_answer` i `turns_select_branch`.

export interface ArenaTurnsApi {
  /**
   * Arena: odpowiedź modelu `model` (`dostawca:model`, przypięty w Routerze) jako nowa gałąź —
   * wiadomość użytkownika → nowe dziecko, odpowiedź agentki → jej rodzeństwo. Zwraca id tury.
   */
  arenaAnswer(sessionId: string, turnId: string, model: string): Promise<string>;
  /** Wybór gałęzi: aktywna ścieżka sesji prowadzi przez tę turę (dalsza rozmowa, głos). */
  selectBranch(sessionId: string, turnId: string): Promise<void>;
}
