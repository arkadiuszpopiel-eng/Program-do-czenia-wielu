// Atrapa IPC Tauri 2 dla okien pobocznych (Szybkie pytanie, pigułka) w Playwright: podstawia
// `window.__TAURI_INTERNALS__`, więc strona idzie ścieżką produkcyjną (`TauriAlfaClient`, `invoke`,
// `listen`) zamiast atrapy `FakeAlfaClient`. Zdarzenia `alfa://events` wysyła sama atrapa.
import type { Page } from '@playwright/test';

export interface TauriMock {
  readonly locale: 'pl' | 'en';
  readonly theme: 'auto' | 'light' | 'dark';
  /**
   * `quick_ask`: najpierw zdarzenia odpowiedzi (tury + tekst + Stop), wynik komendy dopiero po
   * tylu ms — jak w rdzeniu, gdzie paczka zdarzeń może wyprzedzić odpowiedź IPC.
   */
  readonly askDelayMs?: number;
}

interface MockWindow {
  __TAURI_INTERNALS__: unknown;
  __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
}

export async function mockTauri(page: Page, mock: TauriMock): Promise<void> {
  await page.addInitScript((m: TauriMock) => {
    const callbacks: Record<number, (data: unknown) => void> = {};
    const listeners: Record<string, number[]> = {};
    let next = 1;
    const emit = (event: string, payload: unknown): void => {
      for (const id of listeners[event] ?? []) callbacks[id]?.({ event, id, payload });
    };
    const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
    const turn = (id: string, author: string, text: string, status: string) => ({
      id,
      session_id: 's-quick-1',
      parent_id: author === 'user' ? null : 't-user-1',
      author,
      role_id: author === 'user' ? null : 'conductor',
      created_at: '2026-10-04T08:00:00.000Z',
      status,
      text,
      blocks: [],
      thinking: null,
      tools: [],
      approval: null,
      usage: null,
      error: null,
      continues: null,
      addressed_to: null,
      truncated: false,
      heard_prefix: null,
    });
    const win = window as unknown as MockWindow;
    win.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'quick' },
        currentWebview: { windowLabel: 'quick', label: 'quick' },
      },
      transformCallback: (callback: (data: unknown) => void) => {
        const id = next++;
        callbacks[id] = callback;
        return id;
      },
      unregisterCallback: (id: number) => delete callbacks[id],
      invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
        switch (cmd) {
          case 'app_bootstrap':
            return {
              app_version: '0.0.0-e2e',
              locale: m.locale,
              onboarding_done: true,
              machine_name: 'E2E',
              settings: { 'ui.theme': m.theme, 'ui.locale': m.locale },
              layout: null,
              active_session_id: null,
              shortcut_overrides: {},
            };
          case 'plugin:event|listen': {
            const event = String(args['event']);
            const handler = Number(args['handler']);
            listeners[event] = [...(listeners[event] ?? []), handler];
            return handler;
          }
          case 'quick_ask': {
            const sid = 's-quick-1';
            emit('alfa://events', [
              {
                type: 'TurnAppended',
                session_id: sid,
                turn: turn('t-user-1', 'user', String(args['text']), 'complete'),
              },
              {
                type: 'TurnAppended',
                session_id: sid,
                turn: turn('t-alfa-1', 'alfa', '', 'streaming'),
              },
            ]);
            emit('alfa://events', [
              {
                type: 'TextDelta',
                session_id: sid,
                turn_id: 't-alfa-1',
                text: 'Cztery.',
                blocks: [
                  {
                    index: 0,
                    kind: 'text',
                    lang: null,
                    html_sanitized: '<p>Cztery.</p>',
                    closed: true,
                  },
                ],
              },
              { type: 'Stop', session_id: sid, turn_id: 't-alfa-1', reason: 'end' },
            ]);
            await sleep(m.askDelayMs ?? 0);
            return { session_id: sid, user_turn_id: 't-user-1', assistant_turn_id: 't-alfa-1' };
          }
          default:
            return null;
        }
      },
    };
    win.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => undefined };
  }, mock);
}
