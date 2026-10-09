// Przyciski okna głównego. Okno nie ma belki systemowej — pasek tytułu rysuje interfejs, więc
// minimalizację, maksymalizację i zamknięcie wywołuje on sam. W przeglądarce (dev, testy E2E) okna
// nie ma: przyciski są widoczne, a stan „zmaksymalizowane” trzymany jest lokalnie.
import { isTauri } from './index';

export interface WindowControls {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
  /** Powiadamia po zmianie rozmiaru okna; zwraca funkcję wyłączającą. */
  onResized(listener: () => void): Promise<() => void>;
}

function browserControls(): WindowControls {
  let maximized = false;
  const listeners = new Set<() => void>();
  return {
    minimize: async () => {},
    toggleMaximize: async () => {
      maximized = !maximized;
      listeners.forEach((listener) => listener());
    },
    close: async () => {},
    isMaximized: async () => maximized,
    onResized: async (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

async function tauriControls(): Promise<WindowControls> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const current = getCurrentWindow();
  return {
    minimize: () => current.minimize(),
    toggleMaximize: () => current.toggleMaximize(),
    close: () => current.close(),
    isMaximized: () => current.isMaximized(),
    onResized: (listener) => current.onResized(() => listener()),
  };
}

export function windowControls(): Promise<WindowControls> {
  return isTauri() ? tauriControls() : Promise.resolve(browserControls());
}
