// Stan Brokera w oknie głównym: widok z rdzenia (`broker_status`, zdarzenia `BrokerStatus`),
// baner bezpiecznego stanu i ukryte przez użytkownika banery informacyjne.
import type { BrokerApi } from '../api/client-broker';
import type { BrokerStatusView } from '../api/types-broker';
import { type BrokerBanner, brokerBanner, safeState } from '../logic/broker';

export class BrokerState {
  view = $state<BrokerStatusView | null>(null);
  /** Odczyt stanu nie powiódł się (bez widoku — baner „stan nieznany”). */
  failed = $state(false);
  private dismissed = $state<BrokerBanner[]>([]);
  /** Licznik widoków ze zdarzeń — starsza migawka z `load` nie nadpisze nowszego zdarzenia. */
  private applied = 0;

  /** Bezpieczny stan: wszystko, co wymaga zgody, jest odrzucane. */
  get safeState(): boolean {
    return safeState(this.view);
  }

  get banner(): BrokerBanner | null {
    return brokerBanner(this.view, this.dismissed, this.failed);
  }

  apply(view: BrokerStatusView): void {
    this.applied += 1;
    this.view = view;
    this.failed = false;
  }

  dismiss(banner: BrokerBanner): void {
    if (!this.dismissed.includes(banner)) this.dismissed = [...this.dismissed, banner];
  }

  /**
   * Po starcie. Migawka nie nadpisuje widoku ze zdarzenia, które przyszło w trakcie odczytu
   * (audyt A-12: wyścig mógł przywrócić stary stan). Błąd odczytu bez widoku — baner
   * „stan nieznany”; widok ze zdarzenia zostaje.
   */
  async load(api: BrokerApi): Promise<void> {
    const seen = this.applied;
    try {
      const view = await api.status();
      if (this.applied === seen) {
        this.view = view;
        this.failed = false;
      }
    } catch {
      if (this.view === null) this.failed = true;
    }
  }
}
