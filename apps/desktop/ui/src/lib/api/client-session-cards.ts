// Karty rozmów z profilem i projekty (część `SessionsApi`; komendy `sessions_cards`, `projects_*`).
import type { PipelineDef, ProjectInfo, SessionCard } from './types';

export interface SessionCardsApi {
  /**
   * Dane kart dla podanych rozmów (agentki, ostatnia wiadomość jako czysty tekst, model, koszt);
   * rozmowy nieistniejące są pomijane, najwięcej 200 kart naraz.
   */
  cards(sessionIds: readonly string[]): Promise<readonly SessionCard[]>;
  /** Lista projektów z katalogami roboczymi i liczbą rozmów. */
  projectsList(): Promise<readonly ProjectInfo[]>;
  /** Intencja: natywne okno wyboru katalogu projektu (`null` — anulowano). */
  projectsPickDir(): Promise<string | null>;
  /** Dodaje lub aktualizuje projekt (z katalogiem roboczym) i zwraca listę projektów. */
  projectsSave(name: string, workdir: string | null): Promise<readonly ProjectInfo[]>;
  /** Zmienia nazwę projektu i aktualizuje przypisane rozmowy. */
  projectsRename(oldName: string, newName: string): Promise<readonly ProjectInfo[]>;
  /** Usuwa projekt (rozmowy trafiają do „Bez projektu”). */
  projectsRemove(name: string): Promise<readonly ProjectInfo[]>;
  /** Biblioteka pipeline'ów modeli: własne i wbudowane wzorce (`pipelines_list`). */
  pipelinesList(): Promise<readonly PipelineDef[]>;
  /** Zapisuje własny pipeline (`previousName` — zmiana nazwy) i zwraca bibliotekę. */
  pipelinesSave(pipeline: PipelineDef, previousName: string | null): Promise<readonly PipelineDef[]>;
  /** Usuwa własny pipeline (nadpisany wzorzec wraca do wbudowanego). */
  pipelinesRemove(name: string): Promise<readonly PipelineDef[]>;
}
