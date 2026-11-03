// Tryb pracy (poziom autonomii) globalny — jedno źródło dla paska pisania, belki i menu „Tryb
// pracy”. Zgłoszenie właściciela 29.10: bez otwartej rozmowy pasek pokazywał na sztywno
// „Automatycznie”, więc po wyborze „Pełnego dostępu” wyglądało, jakby zmiana nie działała.
// Poziom z Brokera odczytujemy po starcie, po każdej zmianie z menu i po powrocie do okna (zgoda
// w oknie Brokera przychodzi z innego procesu).
import type { PermissionsApi } from '../api/client';
import type { AutonomyLevel } from '../api/types';
import type { ChildLockState, MaxAccess } from '../api/types-hub';

export class AutonomyState {
  /** Poziom globalny z Brokera (`null`, dopóki nie wczytano). */
  global = $state<AutonomyLevel | null>(null);
  /** Włączony „Max dostęp” (plan 50 §8.1): zasięg albo `null` — wyłączony. */
  maxAccess = $state<MaxAccess | null>(null);
  /** Profil Dziecko (plan 50 §4.4); `null` — nie wczytano albo Sędzia go nie obsługuje. */
  child = $state<ChildLockState | null>(null);
  /** Kłódka rodzica zamknięta: wyższe tryby i „Max dostęp” otwiera tylko PIN rodzica. */
  locked = $derived(this.child !== null && this.child.enabled && !this.child.open);
  /** Numer odczytu — spóźniona odpowiedź nie nadpisze nowszej. */
  private seq = 0;

  /** Odczytuje poziom globalny; zwraca go (`null` — odczyt się nie udał, stan bez zmian). */
  async load(api: PermissionsApi): Promise<AutonomyLevel | null> {
    const n = ++this.seq;
    try {
      const [{ global, max_access }, child] = await Promise.all([
        api.get(null),
        // Profil Dziecko nie blokuje odczytu poziomu (stary rdzeń, błąd łącza).
        Promise.resolve()
          .then(() => api.childLock(null))
          .catch(() => null),
      ]);
      if (n === this.seq) {
        this.global = global;
        this.maxAccess = max_access ?? null;
        this.child = child;
      }
      return global;
    } catch {
      return null;
    }
  }
}
