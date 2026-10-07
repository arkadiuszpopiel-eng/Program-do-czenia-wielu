// Atrapa `@tauri-apps/api/core` i `@tauri-apps/api/event` dla generatora fixture'ów: zapisuje nazwę
// komendy i argumenty dokładnie tak, jak wysyła je `TauriAlfaClient` (camelCase), bez IPC.
export interface Invocation {
  readonly command: string;
  readonly args: Record<string, unknown>;
}

export const invocations: Invocation[] = [];

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  invocations.push({ command, args: args ?? {} });
  return undefined as T;
}

export async function listen(): Promise<() => void> {
  return () => undefined;
}

export type UnlistenFn = () => void;

/** Kanał strumienia (`terminal_open`): w IPC serializuje się do identyfikatora `__CHANNEL__:<n>`. */
export class Channel<T> {
  private static next = 1;
  readonly id = Channel.next++;
  onmessage: (message: T) => void = () => undefined;

  toJSON(): string {
    return `__CHANNEL__:${this.id}`;
  }
}

/** Adres protokołu zasobów (`asset:`) — w generatorze tylko kształt, bez IPC. */
export function convertFileSrc(path: string): string {
  return `http://asset.localhost/${encodeURIComponent(path)}`;
}
