// Tryb pracy (poziom autonomii) globalny — jedno źródło dla paska pisania, belki i menu „Tryb
// pracy”. Zgłoszenie właściciela 29.10: bez otwartej rozmowy pasek pokazywał na sztywno
// „Automatycznie”, więc po wyborze „Pełnej automatyki” wyglądało, jakby zmiana nie działała.
// Poziom z Brokera odczytujemy po starcie, po każdej zmianie z menu i po powrocie do okna (zgoda
// w oknie Brokera przychodzi z innego procesu).
import type { PermissionsApi } from '../api/client';
import type { AutonomyLevel } from '../api/types';

export class AutonomyState {
  /** Poziom globalny z Brokera (`null`, dopóki nie wczytano). */
  global = $state<AutonomyLevel | null>(null);
  /** Numer odczytu — spóźniona odpowiedź nie nadpisze nowszej. */
  private seq = 0;

  /** Odczytuje poziom globalny; zwraca go (`null` — odczyt się nie udał, stan bez zmian). */
  async load(api: PermissionsApi): Promise<AutonomyLevel | null> {
    const n = ++this.seq;
    try {
      const { global } = await api.get(null);
      if (n === this.seq) this.global = global;
      return global;
    } catch {
      return null;
    }
  }
}
