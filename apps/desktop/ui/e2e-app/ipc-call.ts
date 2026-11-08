// Wspólne wywołania łącza dla testów prawdziwej aplikacji: odpowiedź sprawdzona ze schematem
// (`call`), czytelna odmowa z kodem (`refused`) i wynik zależny od wyścigu (`settled`).
import type { Page } from '@playwright/test';
import { expect } from './alfa';
import { loadIpcSchema, schemaErrors } from './ipc';

const schema = loadIpcSchema();

export type Reply =
  { ok: true; value: unknown } | { ok: false; error: { code?: string; message?: string } };

export async function raw(
  page: Page,
  command: string,
  args: Record<string, unknown>,
): Promise<Reply> {
  return page.evaluate(
    async ([name, payload]) => {
      const ipc = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      try {
        return { ok: true as const, value: await ipc.invoke(name, payload) };
      } catch (error) {
        return { ok: false as const, error: error as { code?: string; message?: string } };
      }
    },
    [command, args] as const,
  );
}

/** Wywołanie, które ma się udać; wynik zgodny ze schematem polecenia. */
export async function call<T = unknown>(
  page: Page,
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const spec = schema.commands[command];
  expect(spec, `polecenie ${command} jest w schemacie`).toBeTruthy();
  const reply = await raw(page, command, args);
  expect(reply.ok, `${command}: ${reply.ok ? '' : JSON.stringify(reply.error)}`).toBe(true);
  const value = reply.ok ? (reply.value ?? null) : null;
  if (spec?.result) {
    expect(schemaErrors(schema, spec.result, value as never), `${command}: schemat`).toEqual([]);
  }
  return value as T;
}

/** Wywołanie, które ma zostać odrzucone czytelnym błędem z kodem. */
export async function refused(
  page: Page,
  command: string,
  args: Record<string, unknown>,
): Promise<string> {
  const reply = await raw(page, command, args);
  expect(reply.ok, `${command} powinno zostać odrzucone`).toBe(false);
  const error = reply.ok ? {} : reply.error;
  expect(typeof error.code, `${command}: błąd ma kod`).toBe('string');
  expect((error.message ?? '').length, `${command}: błąd ma opis`).toBeGreaterThan(3);
  return error.code ?? '';
}

/**
 * Wywołanie, którego wynik zależy od wyścigu z rdzeniem (zadanie bez modelu kończy się samo):
 * sukces albo czytelna odmowa z kodem — nigdy cisza ani błąd bez opisu.
 */
export async function settled(
  page: Page,
  command: string,
  args: Record<string, unknown>,
): Promise<void> {
  const reply = await raw(page, command, args);
  if (reply.ok) return;
  expect(typeof reply.error.code, `${command}: błąd ma kod`).toBe('string');
  expect((reply.error.message ?? '').length, `${command}: błąd ma opis`).toBeGreaterThan(3);
}
