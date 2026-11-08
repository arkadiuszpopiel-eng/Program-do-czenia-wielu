// Stan aktualizacji w oknie głównym: widok z rdzenia (zdarzenia `UpdateStatus`), baner
// „Uruchom ponownie, aby zaktualizować" i „Co nowego" pokazywane raz po aktualizacji.
import type { UpdatesApi } from '../api/client-updates';
import type { UpdatesView, WhatsNew } from '../api/types-updates';

export class UpdatesState {
  view = $state<UpdatesView | null>(null);
  whatsNew = $state<WhatsNew | null>(null);
  /** Baner ukryty do następnej zmiany przygotowanej wersji. */
  bannerHiddenFor = $state<string | null>(null);

  /** Przygotowana wersja czeka na ponowne uruchomienie (aktualizacja albo przywrócenie). */
  get readyVersion(): string | null {
    return this.view?.phase === 'ready' ? this.view.ready : null;
  }

  get showBanner(): boolean {
    const ready = this.readyVersion;
    return ready !== null && this.bannerHiddenFor !== ready;
  }

  apply(view: UpdatesView): void {
    this.view = view;
  }

  /**
   * Stan ze zdarzenia rdzenia. Zdarzenia nie niosą powodu blokady ponownego uruchomienia (rdzeń
   * liczy go tylko na żądanie), więc zachowujemy ostatni znany — inaczej przycisk „Uruchom ponownie"
   * odblokowywał się po każdym zdarzeniu, a kliknięcie kończyło się błędem.
   */
  applyEvent(view: UpdatesView): void {
    this.fromEvent = true;
    const blocked = view.restart_blocked ?? this.view?.restart_blocked ?? null;
    this.view = { ...view, restart_blocked: view.phase === 'ready' ? blocked : null };
  }

  /**
   * Powód blokady ponownego uruchomienia prosto z rdzenia. Zdarzenia go nie niosą, a zmienia się
   * bez zdarzeń (koniec rozmowy głosowej, koniec zadania agentki) — bez odpytania przycisk
   * zostawał wyłączony albo włączony wbrew stanowi.
   */
  async refreshRestartBlock(api: UpdatesApi): Promise<void> {
    try {
      const fresh = await api.status();
      if (this.view?.phase === 'ready' && fresh.phase === 'ready') {
        this.view = { ...this.view, restart_blocked: fresh.restart_blocked ?? null };
      }
    } catch {
      // Brak odpowiedzi: zostaje ostatni znany stan.
    }
  }

  /** Czy stan przyszedł zdarzeniem od ostatniego `load` (wtedy odpowiedź `status` jest starsza). */
  private fromEvent = false;

  hideBanner(): void {
    this.bannerHiddenFor = this.readyVersion;
  }

  /** Po starcie: stan i „Co nowego" (błąd = brak modułu; UI działa dalej bez aktualizacji). */
  async load(api: UpdatesApi): Promise<void> {
    this.fromEvent = false;
    try {
      const view = await api.status();
      // Zdarzenie, które przyszło w trakcie pytania o stan, jest nowsze niż odpowiedź.
      if (!this.fromEvent) this.view = view;
    } catch {
      if (!this.fromEvent) this.view = null;
    }
    // Osobno: błąd „Co nowego" nie może skasować informacji, że wersja czeka na restart.
    try {
      this.whatsNew = await api.whatsNew();
    } catch {
      this.whatsNew = null;
    }
  }

  async dismissWhatsNew(api: UpdatesApi): Promise<void> {
    this.whatsNew = null;
    await api.dismissWhatsNew();
  }
}
