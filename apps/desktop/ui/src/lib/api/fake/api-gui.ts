// Atrapa: panel „Ekran" (Delta steruje Notatnikiem; zrzut z zamaskowanym oknem Alfy jako SVG;
// przejęcie i oddanie sterowania; prośba o podgląd pulpitu → okno Brokera) i wbudowany terminal
// (baner profilu, echo wejścia, koniec procesu przy zamknięciu). Strumień terminala idzie tylko
// do `onFrame` — atrapa nie emituje go zdarzeniami.
import type { AlfaClient } from '../client';
import type {
  GuiAction,
  GuiScreenshot,
  GuiStatus,
  TerminalFrame,
  TerminalProfileId,
  TerminalSession,
} from '../types-work';
import type { FakeCore } from './core';

const SHOT_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360">' +
  '<rect width="640" height="360" fill="#2b3a4a"/>' +
  '<rect x="24" y="24" width="360" height="240" rx="6" fill="#f4f4f4"/>' +
  '<rect x="24" y="24" width="360" height="24" rx="6" fill="#d9d9d9"/>' +
  '<text x="36" y="41" font-size="12" font-family="sans-serif" fill="#222">Bez tytulu - Notatnik</text>' +
  '<text x="40" y="80" font-size="13" font-family="monospace" fill="#222">Lista zakupow:</text>' +
  '<rect x="408" y="40" width="208" height="280" fill="#000"/>' +
  '<text x="512" y="184" font-size="12" font-family="sans-serif" fill="#bbb" text-anchor="middle">zamaskowano</text>' +
  '</svg>';

/** 1:1 obraz dla `data:` URL (jak PNG z portu zrzutów — tylko w pamięci). */
export const FAKE_SHOT_URL = `data:image/svg+xml;base64,${btoa(SHOT_SVG)}`;

export class FakeGui {
  private status: GuiStatus;
  private seq = 3;
  private readonly sessions = new Map<number, { profile: TerminalProfileId; open: boolean }>();
  private readonly sinks = new Map<number, (frame: TerminalFrame) => void>();
  private nextTerminal = 1;

  constructor(private readonly core: FakeCore) {
    const empty = core.scenario === 'empty' || core.scenario === 'first-run';
    const at = (min: number) => new Date(core.scheduler.now() - min * 60_000).toISOString();
    const action = (id: number, min: number, patch: Partial<GuiAction>): GuiAction => ({
      id,
      at: at(min),
      session_id: 's-q3',
      agent: 'delta',
      tool: 'window_focus',
      title: 'Fokus okna',
      target: 'notepad.exe',
      status: 'ok',
      summary: 'Fokus okna',
      duration_ms: 120,
      ...patch,
    });
    this.status = {
      available: true,
      reason: null,
      control: empty
        ? null
        : { session_id: 's-q3', agent: 'delta', tool: 'input_type_text', since: at(0) },
      taken_over: false,
      actions: empty
        ? []
        : [
            action(3, 0, {
              tool: 'input_type_text',
              title: 'Wpisz tekst',
              status: 'running',
              summary: 'Wpisz tekst (24 znaków)',
              duration_ms: null,
            }),
            action(2, 1, {
              tool: 'screen_capture',
              title: 'Zrzut ekranu',
              summary: 'Zrzut ekranu 640×360, zamaskowano 1',
            }),
            action(1, 2, {}),
          ],
      screenshot: empty
        ? null
        : {
            at: at(1),
            session_id: 's-q3',
            agent: 'delta',
            width: 640,
            height: 360,
            masked: 1,
            black_frame: false,
          },
    };
  }

  private announce(): GuiStatus {
    this.core.emit([{ type: 'GuiActivity', status: this.status }]);
    return this.status;
  }

  api(): AlfaClient['gui'] {
    const core = this.core;
    return {
      status: () => core.reply(this.status),
      screenshot: () => {
        const info = this.status.screenshot;
        const shot: GuiScreenshot | null = info ? { info, data_url: FAKE_SHOT_URL } : null;
        return core.reply(shot);
      },
      stop: () => {
        this.seq++;
        const running = this.status.actions.map((a) =>
          a.status === 'running'
            ? { ...a, status: 'cancelled' as const, summary: `${a.title} — anulowano` }
            : a,
        );
        this.status = { ...this.status, control: null, taken_over: true, actions: running };
        return core.reply(this.announce());
      },
      release: () => {
        this.status = { ...this.status, taken_over: false };
        return core.reply(this.announce());
      },
      desktopGrant: (sessionId, agent) => {
        if (!core.session(sessionId)) return Promise.reject(new Error('Nieznana sesja.'));
        if (!/^[a-z0-9-]{1,32}$/.test(agent)) return Promise.reject(new Error('Nieznana agentka.'));
        return core.reply({ status: 'opened_broker' as const, request_id: `${++this.seq}` });
      },
    };
  }

  private frame(id: number, frame: TerminalFrame): void {
    const sink = this.sinks.get(id);
    if (sink) queueMicrotask(() => sink(frame));
  }

  private out(id: number, text: string): void {
    const bytes = new TextEncoder().encode(text);
    let raw = '';
    for (const b of bytes) raw += String.fromCharCode(b);
    this.frame(id, { kind: 'output', data_b64: btoa(raw) });
  }

  terminalApi(): AlfaClient['terminal'] {
    const core = this.core;
    const banner: Record<TerminalProfileId, string> = {
      shell: 'PowerShell 7.5 (atrapa)\r\nPS C:\\Users\\ala> ',
      cmd: 'Microsoft Windows (atrapa)\r\nC:\\Users\\ala>',
      claude_login: 'Claude Code (atrapa) — wpisz /login, aby się zalogować\r\n> ',
      codex_login: 'Codex (atrapa) — logowanie: postępuj zgodnie z instrukcją\r\n> ',
    };
    const session = (id: number): TerminalSession => {
      const s = this.sessions.get(id);
      return { id, profile: s?.profile ?? 'shell', pid: 4000 + id, alive: s?.open ?? false };
    };
    return {
      open: (profile, cols, rows, _cwd, onFrame) => {
        if (cols < 2 || rows < 1)
          return Promise.reject(new Error('Rozmiar terminala poza zakresem.'));
        const open = [...this.sessions.values()].filter((s) => s.open).length;
        if (open >= 3) return Promise.reject(new Error('Otwarto już 3 terminale.'));
        const id = this.nextTerminal++;
        this.sessions.set(id, { profile, open: true });
        this.sinks.set(id, onFrame);
        this.out(id, banner[profile]);
        return core.reply(session(id));
      },
      input: (terminal, dataB64) => {
        const s = this.sessions.get(terminal);
        if (!s?.open) return Promise.reject(new Error(`Terminal ${terminal} nie istnieje.`));
        const text = atob(dataB64);
        this.out(terminal, text.replace(/\r/g, '\r\n> '));
        return core.reply(undefined);
      },
      resize: (terminal, cols, rows) => {
        if (!this.sessions.get(terminal)?.open || cols < 2 || rows < 1)
          return Promise.reject(new Error('Nie można zmienić rozmiaru terminala.'));
        return core.reply(undefined);
      },
      close: (terminal) => {
        const s = this.sessions.get(terminal);
        if (!s) return Promise.reject(new Error(`Terminal ${terminal} nie istnieje.`));
        this.frame(terminal, { kind: 'exit', code: 0 });
        this.sessions.delete(terminal);
        this.sinks.delete(terminal);
        return core.reply(undefined);
      },
      list: () => core.reply([...this.sessions.keys()].map(session)),
    };
  }
}
