// Interfejs Tablicy (część `AlfaClient`; komendy `board_*`).
import type { BoardItem } from './types-board';

export interface BoardApi {
  list(): Promise<readonly BoardItem[]>;
  /** Przypina odpowiedź agentki (kopia treści); pełna tablica — błąd z powodem. */
  pin(sessionId: string, turnId: string): Promise<readonly BoardItem[]>;
  unpin(id: string): Promise<readonly BoardItem[]>;
}
