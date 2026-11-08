// Atrapa: aktualizacje i „O programie". Deterministyczna (zegar atrapy): „Sprawdź teraz" znajduje
// 0.2.0, pobieranie 4 krokami co 250 ms (postęp w zdarzeniach `UpdateStatus`), weryfikacja,
// instalacja, „Uruchom ponownie" symuluje restart przez launcher (bieżąca = przygotowana,
// „Co nowego" raz), „Przywróć poprzednią wersję", restart zablokowany w trakcie rozmowy głosowej.
// Scenariusz „offline": sprawdzanie kończy się błędem sieci.
import type { UpdatesApi } from '../client-updates';
import type {
  AboutInfo,
  ChangeLogEntry,
  LicenseEntry,
  UpdateChannel,
  UpdateMode,
  UpdatesView,
  WhatsNew,
} from '../types-updates';
import type { FakeCore } from './core';

export const FAKE_CURRENT = '0.1.0-f1';
export const FAKE_NEXT = '0.2.0';
export const FAKE_TOTAL = 48_234_496;
export const FAKE_STEP_MS = 250;
/** Historia zmian atrapy: trzy wersje, wszystkie wagi, wpis bez pozycji i wpis bez daty. */
export const FAKE_HISTORY: readonly ChangeLogEntry[] = [
  {
    version: '0.1.0-f1',
    date: '2026-10-04',
    summary: 'Klucze API pod opieką i pewniejsze kopie zapasowe.',
    items: [
      {
        section: 'Kopie zapasowe',
        severity: 'critical',
        text: 'Kopia zapasowa nie powstaje w wersji nieszyfrowanej, gdy nie da się odczytać hasła.',
      },
      {
        section: 'Konta i klucze API',
        severity: 'serious',
        text: 'Zły klucz u Google i xAI jest pokazywany jako zły klucz, a nie „błąd połączenia”.',
      },
      {
        section: 'Konta i klucze API',
        severity: 'normal',
        text: 'Sprawdzanie kluczy w tle: co 24 godziny Alfa sprawdza każde włączone konto, bez zużycia tokenów.',
      },
      {
        section: 'Konta i klucze API',
        severity: 'normal',
        text: 'Wiek klucza: przy każdym koncie widać, ile dni ma zapisany klucz.',
      },
    ],
  },
  {
    version: '0.0.9',
    date: '2026-09-28',
    summary: 'Porządki w interfejsie.',
    items: [
      {
        section: 'Interfejs',
        severity: 'serious',
        text: 'Ekran ładowania nie może już zostać na zawsze: błąd startu jest na nim pokazywany.',
      },
      {
        section: 'Interfejs',
        severity: 'normal',
        text: 'Szczegóły pod odpowiedzią (model, tokeny, koszt, czas) można wyłączyć w Ustawieniach → Wygląd.',
      },
      {
        section: 'Głos',
        severity: 'normal',
        text: 'Głośność mowy: głośność głosu agentek niezależnie od głośności systemu.',
      },
    ],
  },
  { version: '0.0.8', date: null, summary: 'Pierwszy stan uruchamialny.', items: [] },
];

/** Długa historia do podglądu zakładki „Starsze” (`?history=long`): atrapa + `extra` starszych wersji. */
export function longHistory(extra: number): readonly ChangeLogEntry[] {
  const older = Array.from({ length: extra }, (_, index): ChangeLogEntry => {
    const n = extra - index;
    return {
      version: `0.0.7-p${n}`,
      date: null,
      summary: `Wersja próbna ${n}.`,
      items: [
        {
          section: 'Porządki',
          severity: n % 5 === 0 ? 'serious' : 'normal',
          text: `Poprawka numer ${n} z okresu prób.`,
        },
      ],
    };
  });
  return [...FAKE_HISTORY, ...older];
}

/** Wpis nowej wersji — w dzienniku dopiero po aktualizacji (jak w prawdziwym programie). */
export const FAKE_NEXT_ENTRY: ChangeLogEntry = {
  version: FAKE_NEXT,
  date: '2026-10-12',
  summary: 'Aktualizacje z wznawianiem pobierania i strona „O programie".',
  items: [
    {
      section: 'Aktualizacje',
      severity: 'serious',
      text: 'Aktualizacje z wznawianiem pobierania i automatycznym powrotem do poprzedniej wersji.',
    },
    {
      section: 'Interfejs',
      severity: 'normal',
      text: 'Strona „O programie" z licencjami zależności.',
    },
    { section: 'Interfejs', severity: 'normal', text: 'Szybsze wyszukiwanie w sesjach.' },
  ],
};

function historyFromUrl(): readonly ChangeLogEntry[] {
  if (typeof location === 'undefined') return FAKE_HISTORY;
  const long = new URLSearchParams(location.search).get('history') === 'long';
  return long ? longHistory(14) : FAKE_HISTORY;
}

const NOTES = `Co nowego w ${FAKE_NEXT}:
- Aktualizacje z wznawianiem pobierania i automatycznym powrotem do poprzedniej wersji.
- Strona „O programie" z licencjami zależności.
- Szybsze wyszukiwanie w sesjach.`;

const LICENSES: readonly LicenseEntry[] = [
  { name: 'minisign-verify', version: '0.3.0', license: 'MIT', source: 'cargo' },
  { name: 'reqwest', version: '0.12.28', license: 'MIT OR Apache-2.0', source: 'cargo' },
  { name: 'tauri', version: '2.12.0', license: 'Apache-2.0 OR MIT', source: 'cargo' },
  { name: 'zip', version: '8.6.0', license: 'MIT', source: 'cargo' },
  { name: 'bits-ui', version: '2.19.3', license: 'MIT', source: 'npm' },
  { name: 'svelte', version: '5.57.1', license: 'MIT', source: 'npm' },
];

function channelOf(value: unknown): UpdateChannel {
  return value === 'preview' || value === 'beta' ? 'beta' : 'stable';
}

function modeOf(value: unknown): UpdateMode {
  return value === 'auto' || value === 'manual' ? value : 'ask';
}

export class FakeUpdates {
  private view: UpdatesView;
  private news: WhatsNew | null = null;
  private timer: number | null = null;

  constructor(private readonly core: FakeCore) {
    this.view = {
      phase: 'idle',
      current: FAKE_CURRENT,
      channel: 'stable',
      mode: 'ask',
      available: null,
      progress: null,
      ready: null,
      previous: null,
      last_check: null,
      error: null,
      restart_blocked: null,
    };
  }

  private snapshot(): UpdatesView {
    const settings = this.core.settings;
    return {
      ...this.view,
      channel: channelOf(settings['updates.channel']),
      mode: modeOf(settings['updates.mode']),
      restart_blocked: this.core.voice.active
        ? {
            pl: 'Trwa rozmowa głosowa — zakończ ją najpierw.',
            en: 'A voice conversation is active — end it first.',
          }
        : null,
    };
  }

  private set(patch: Partial<UpdatesView>): UpdatesView {
    this.view = { ...this.view, ...patch };
    const status = this.snapshot();
    this.core.emit([{ type: 'UpdateStatus', status }]);
    return status;
  }

  private step(downloaded: number): void {
    if (downloaded < FAKE_TOTAL) {
      this.set({ progress: { downloaded, total: FAKE_TOTAL, resumed: false } });
      this.timer = this.core.scheduler.setTimeout(
        () => this.step(Math.min(FAKE_TOTAL, downloaded + FAKE_TOTAL / 4)),
        FAKE_STEP_MS,
      );
      return;
    }
    this.set({ phase: 'verifying', progress: { downloaded, total: FAKE_TOTAL, resumed: false } });
    this.timer = this.core.scheduler.setTimeout(() => {
      this.set({ phase: 'installing' });
      this.timer = this.core.scheduler.setTimeout(() => {
        this.timer = null;
        this.set({
          phase: 'ready',
          ready: FAKE_NEXT,
          previous: this.view.current,
          available: null,
          progress: null,
        });
      }, FAKE_STEP_MS);
    }, FAKE_STEP_MS);
  }

  api(): UpdatesApi {
    const core = this.core;
    return {
      status: () => core.reply(this.snapshot()),
      check: () => {
        if (core.scenario === 'offline') {
          this.set({ phase: 'failed', error: 'sieć: brak połączenia (atrapa)' });
          return Promise.reject(new Error('Brak połączenia z serwerem wydań.'));
        }
        const newer = this.view.current !== FAKE_NEXT && this.view.ready === null;
        return core.reply(
          this.set({
            phase: newer ? 'available' : this.view.ready ? 'ready' : 'up_to_date',
            available: newer ? { version: FAKE_NEXT, notes: NOTES } : null,
            last_check: core.isoNow(),
            error: null,
          }),
        );
      },
      download: () => {
        const resumable = this.view.phase === 'failed' && this.view.available;
        if (this.view.phase !== 'available' && !resumable) {
          return Promise.reject(
            new Error('Brak aktualizacji do pobrania — najpierw sprawdź dostępność.'),
          );
        }
        const from = this.view.progress?.downloaded ?? 0;
        const status = this.set({ phase: 'downloading', error: null });
        this.step(from);
        return core.reply(status);
      },
      cancel: () => {
        if (this.timer !== null) core.scheduler.clearTimeout(this.timer);
        this.timer = null;
        return core.reply(
          this.view.phase === 'downloading' ? this.set({ phase: 'available' }) : this.snapshot(),
        );
      },
      restart: () => {
        const ready = this.view.ready;
        if (!ready)
          return Promise.reject(new Error('Nie ma przygotowanej wersji do uruchomienia.'));
        const blocked = this.snapshot().restart_blocked;
        if (blocked) return Promise.reject(new Error(blocked.pl));
        // Atrapa „restartu": launcher uruchamia przygotowaną wersję, „Co nowego" raz.
        const previous = this.view.current;
        if (ready === FAKE_NEXT) this.news = { version: ready, notes: NOTES };
        this.set({ phase: 'up_to_date', current: ready, ready: null, previous });
        return core.reply(undefined);
      },
      rollback: () => {
        if (this.view.ready && this.view.ready !== this.view.previous) {
          return core.reply(this.set({ phase: 'idle', ready: null }));
        }
        const previous = this.view.previous;
        if (!previous) return Promise.reject(new Error('Brak poprzedniej wersji do przywrócenia.'));
        return core.reply(
          this.set({ phase: 'ready', ready: previous, previous: this.view.current }),
        );
      },
      about: (): Promise<AboutInfo> =>
        core.reply({
          version: this.view.current,
          channel: channelOf(core.settings['updates.channel']),
          build_date: null,
          commit: null,
          target: 'windows-x86_64',
          updates_configured: true,
          licenses_generated_at: '2026-10-03',
          licenses: LICENSES,
        }),
      whatsNew: () => core.reply(core.settings['updates.whats_new'] === false ? null : this.news),
      dismissWhatsNew: () => {
        this.news = null;
        return core.reply(undefined);
      },
      history: () =>
        core.reply(
          this.view.current === FAKE_NEXT
            ? [FAKE_NEXT_ENTRY, ...historyFromUrl()]
            : historyFromUrl(),
        ),
    };
  }
}
