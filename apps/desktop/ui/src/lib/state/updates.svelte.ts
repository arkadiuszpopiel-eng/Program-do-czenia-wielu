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

  hideBanner(): void {
    this.bannerHiddenFor = this.readyVersion;
  }

  /** Po starcie: stan i „Co nowego" (błąd = brak modułu; UI działa dalej bez aktualizacji). */
  async load(api: UpdatesApi): Promise<void> {
    try {
      this.view = await api.status();
      this.whatsNew = await api.whatsNew();
    } catch {
      this.view = null;
    }
  }

  async dismissWhatsNew(api: UpdatesApi): Promise<void> {
    this.whatsNew = null;
    await api.dismissWhatsNew();
  }
}
