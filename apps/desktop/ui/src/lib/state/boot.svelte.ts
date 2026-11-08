// Dziennik rozruchu w programie: po starcie UI czekamy na zamknięcie okna rozruchu (`boot_report`,
// ok. 10 s po złożeniu rdzenia) i pokazujemy werdykt — „Rozruch OK”, „z uwagami” albo „z problemem”
// (ten ostatni zostaje, z przyciskiem „Pokaż dziennik” → Ustawienia → Zdrowie systemu).
import type { BootReport } from '../api/types-boot';
import { bootText } from '../logic/boot-texts';
import type { AppState } from './app.svelte';

/** Odstęp pytań o raport, dopóki okno rozruchu jest otwarte. */
export const BOOT_POLL_MS = 2000;
/** Najwięcej pytań (ok. minuty) — potem werdykt z tego, co jest. */
export const BOOT_MAX_POLLS = 30;
/** Strona ustawień z dziennikiem rozruchu (Zdrowie systemu). */
export const BOOT_PAGE = 'improve';

export class BootState {
  report = $state<BootReport | null>(null);
}

export const boot = new BootState();

const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/** Sekundy rozruchu po polsku/angielsku (jedno miejsce po przecinku). */
export function bootSeconds(locale: string, ms: number): string {
  return new Intl.NumberFormat(locale === 'pl' ? 'pl-PL' : 'en-US', {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  }).format(ms / 1000);
}

/** Po starcie: raport rozruchu → `boot.report` i toast z werdyktem. */
export async function watchBoot(app: AppState, sleep = wait): Promise<void> {
  for (let i = 0; i < BOOT_MAX_POLLS; i++) {
    try {
      boot.report = await app.client.system.bootReport();
    } catch {
      return;
    }
    if (boot.report.finished) break;
    await sleep(BOOT_POLL_MS);
  }
  const r = boot.report;
  // Werdykt tylko po zamkniętym oknie rozruchu (w programie zawsze po ok. 10 s).
  if (!r?.finished) return;
  const t = (key: Parameters<typeof bootText>[1], p = {}) => bootText(app.i18n.locale, key, p);
  const params = {
    s: bootSeconds(app.i18n.locale, r.elapsed_ms),
    w: r.warnings,
    e: r.errors,
  };
  const showLog = { actionLabel: t('showLog'), onAction: () => app.openSettings(BOOT_PAGE) };
  if (r.verdict === 'errors') {
    app.toasts.show({
      kind: 'error',
      message: t('toast.errors', params),
      timeoutMs: 0,
      ...showLog,
    });
  } else if (r.verdict === 'warnings') {
    app.toasts.show({
      kind: 'info',
      message: t('toast.warnings', params),
      timeoutMs: 6000,
      ...showLog,
    });
  } else {
    app.toasts.show({ kind: 'success', message: t('toast.ok', params), timeoutMs: 4000 });
  }
}
