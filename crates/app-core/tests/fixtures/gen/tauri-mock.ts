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
