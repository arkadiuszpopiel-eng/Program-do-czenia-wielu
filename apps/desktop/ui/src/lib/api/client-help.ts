// Interfejs wbudowanej instrukcji (część `AlfaClient`; komendy `help_*`).
import type { HelpHit, HelpPage, HelpTopic } from './types-help';

export interface HelpApi {
  topics(): Promise<readonly HelpTopic[]>;
  page(id: string): Promise<HelpPage>;
  search(query: string): Promise<readonly HelpHit[]>;
}
