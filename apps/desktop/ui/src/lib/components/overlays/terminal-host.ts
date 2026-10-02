// Emulator terminala (@xterm/xterm, MIT) — moduł ładowany wyłącznie z dialogu terminala
// (`import()`), więc nie trafia do paczki startowej. Fonty systemowe (Cascadia Mono), kolory
// z tokenów motywu. Strumień przychodzi z `Channel` jako bajty (UTF-8 dekoduje emulator).
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';

export interface TerminalHost {
  readonly cols: number;
  readonly rows: number;
  write(data: Uint8Array | string): void;
  /** Dopasowanie do kontenera; `true` — zmienił się rozmiar w znakach. */
  fit(): boolean;
  focus(): void;
  dispose(): void;
}

export interface HostOptions {
  readonly label: string;
  onData(data: string): void;
  onBinary(data: string): void;
}

function token(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

export function createTerminal(container: HTMLElement, options: HostOptions): TerminalHost {
  const term = new Terminal({
    fontFamily: '"Cascadia Mono", Consolas, monospace',
    fontSize: 13,
    cursorBlink: true,
    scrollback: 5_000,
    screenReaderMode: false,
    theme: {
      background: token('--alfa-color-bg', '#1e1e1e'),
      foreground: token('--alfa-color-text', '#e6e6e6'),
      cursor: token('--alfa-color-focus', '#4f8cff'),
      selectionBackground: token('--alfa-color-border-strong', '#264f78'),
    },
  });
  const fitter = new FitAddon();
  term.loadAddon(fitter);
  term.open(container);
  term.textarea?.setAttribute('aria-label', options.label);
  const data = term.onData(options.onData);
  const binary = term.onBinary(options.onBinary);
  let last = { cols: term.cols, rows: term.rows };
  return {
    get cols() {
      return term.cols;
    },
    get rows() {
      return term.rows;
    },
    write: (chunk) => term.write(chunk),
    fit: () => {
      try {
        fitter.fit();
      } catch {
        return false;
      }
      const changed = term.cols !== last.cols || term.rows !== last.rows;
      last = { cols: term.cols, rows: term.rows };
      return changed;
    },
    focus: () => term.focus(),
    dispose: () => {
      data.dispose();
      binary.dispose();
      term.dispose();
    },
  };
}
