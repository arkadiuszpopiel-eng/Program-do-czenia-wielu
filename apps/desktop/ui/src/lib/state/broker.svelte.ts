// Stan Brokera w oknie głównym: widok z rdzenia (`broker_status`, zdarzenia `BrokerStatus`),
// baner bezpiecznego stanu i ukryte przez użytkownika banery informacyjne.
import type { BrokerApi } from '../api/client-broker';
import type { BrokerStatusView } from '../api/types-broker';
import { type BrokerBanner, brokerBanner, safeState } from '../logic/broker';

export class BrokerState {
  view = $state<BrokerStatusView | null>(null);
  private dismissed = $state<BrokerBanner[]>([]);

  /** Bezpieczny stan: wszystko, co wymaga zgody, jest odrzucane. */
  get safeState(): boolean {
    return safeState(this.view);
  }

  get banner(): BrokerBanner | null {
    return brokerBanner(this.view, this.dismissed);
  }

  apply(view: BrokerStatusView): void {
    this.view = view;
  }

  dismiss(banner: BrokerBanner): void {
    if (!this.dismissed.includes(banner)) this.dismissed = [...this.dismissed, banner];
  }

  /** Po starcie (błąd = rdzeń bez komendy; baner się nie pojawi). */
  async load(api: BrokerApi): Promise<void> {
    try {
      this.view = await api.status();
    } catch {
      this.view = null;
    }
  }
}
