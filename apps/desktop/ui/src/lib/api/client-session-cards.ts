// Karty rozmów z profilem (część `SessionsApi`; komenda `sessions_cards`).
import type { SessionCard } from './types';

export interface SessionCardsApi {
  /**
   * Dane kart dla podanych rozmów (agentki, ostatnia wiadomość jako czysty tekst, model, koszt);
   * rozmowy nieistniejące są pomijane, najwięcej 200 kart naraz.
   */
  cards(sessionIds: readonly string[]): Promise<readonly SessionCard[]>;
}
