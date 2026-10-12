// Łącze z rdzeniem w teście prawdziwej aplikacji (docs/plan/24 §2.3):
//  • ślad poleceń — każde `invoke` okna trafia do `out/ipc-seen.txt` (pokrycie liczy
//    `scripts/ipc-coverage.mjs`); kod programu nie jest zmieniany, śledzenie wchodzi przez CDP;
//  • przegląd poleceń odczytu — odpowiedź prawdziwego rdzenia sprawdzana ze schematem
//    `packages/schemas/ipc.v1.json` generowanym z typów Rust.
import { appendFileSync, mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Page } from '@playwright/test';

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
interface Schema {
  $ref?: string;
  type?: string | string[];
  properties?: Record<string, Schema>;
  required?: string[];
  items?: Schema;
  enum?: Json[];
  const?: Json;
  anyOf?: Schema[];
  oneOf?: Schema[];
  additionalProperties?: boolean | Schema;
  minimum?: number;
  maximum?: number;
}
interface IpcSchema {
  $defs: Record<string, Schema>;
  commands: Record<string, { args: { name: string }[]; result: Schema | null }>;
}

const SCHEMA_PATH = fileURLToPath(
  new URL('../../../../packages/schemas/ipc.v1.json', import.meta.url),
);

export function loadIpcSchema(): IpcSchema {
  return JSON.parse(readFileSync(SCHEMA_PATH, 'utf8')) as IpcSchema;
}

/**
 * Polecenia odczytu bez argumentów — bezpieczne do wywołania w każdej chwili. Poza listą zostają
 * polecenia, które coś zmieniają albo uruchamiają (aktualizacja, pomiar sprzętu, skan zdrowia,
 * zrzut ekranu) oraz odczyt schowka użytkownika.
 */
export const READ_SWEEP: readonly string[] = [
  'accounts_catalog',
  'accounts_list',
  'app_bootstrap',
  'backups_status',
  'broker_status',
  'builder_library',
  'builder_policy',
  'cameras_state',
  'device_gpu_live',
  'device_profile',
  'evals_list',
  'gui_status',
  'health_report',
  'improver_list',
  'marshal_report',
  'marshal_state',
  'memory_scopes',
  'memory_status',
  'models_bundles',
  'models_list',
  'models_local_list',
  'plugins_list',
  'search_reindex_status',
  'sessions_list',
  'settings_schema',
  'settings_values',
  'skills_list',
  'system_status',
  'tasks_list',
  'terminal_list',
  'triggers_list',
  'updates_about',
  'updates_history',
  'updates_status',
  'updates_whats_new',
  'voice_devices',
  'voice_features',
  'voice_status',
];

const typeOf = (value: Json): string => {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  if (typeof value === 'number') return Number.isInteger(value) ? 'integer' : 'number';
  return typeof value;
};

const matchesType = (wanted: string, value: Json): boolean => {
  const actual = typeOf(value);
  return wanted === actual || (wanted === 'number' && actual === 'integer');
};

/** Niezgodności wartości ze schematem (pusta lista = zgodna). Obsługuje słowa używane w `ipc.v1.json`. */
export function schemaErrors(root: IpcSchema, schema: Schema, value: Json, path = '$'): string[] {
  if (schema.$ref) {
    const name = schema.$ref.replace('#/$defs/', '');
    const target = root.$defs[name];
    return target ? schemaErrors(root, target, value, path) : [`${path}: brak definicji ${name}`];
  }
  const errors: string[] = [];
  for (const [key, list] of [
    ['anyOf', schema.anyOf],
    ['oneOf', schema.oneOf],
  ] as const) {
    if (!list) continue;
    const results = list.map((option) => schemaErrors(root, option, value, path));
    const passing = results.filter((r) => r.length === 0).length;
    if (passing === 0) {
      const closest = results.reduce((a, b) => (b.length < a.length ? b : a));
      errors.push(`${path}: żaden wariant ${key} nie pasuje (${closest[0] ?? '—'})`);
    } else if (key === 'oneOf' && passing > 1) {
      errors.push(`${path}: pasuje ${passing} wariantów oneOf`);
    }
  }
  if (schema.const !== undefined && JSON.stringify(schema.const) !== JSON.stringify(value)) {
    errors.push(`${path}: oczekiwano ${JSON.stringify(schema.const)}`);
  }
  if (schema.enum && !schema.enum.some((e) => JSON.stringify(e) === JSON.stringify(value))) {
    errors.push(`${path}: ${JSON.stringify(value)} spoza listy wartości`);
  }
  if (schema.type) {
    const wanted = Array.isArray(schema.type) ? schema.type : [schema.type];
    if (!wanted.some((t) => matchesType(t, value))) {
      errors.push(`${path}: typ ${typeOf(value)}, oczekiwano ${wanted.join(' | ')}`);
      return errors;
    }
  }
  if (typeof value === 'number') {
    if (schema.minimum !== undefined && value < schema.minimum) errors.push(`${path}: < minimum`);
    if (schema.maximum !== undefined && value > schema.maximum) errors.push(`${path}: > maximum`);
  }
  if (Array.isArray(value) && schema.items) {
    value.forEach((item, index) => {
      if (schema.items) errors.push(...schemaErrors(root, schema.items, item, `${path}[${index}]`));
    });
  }
  if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
    for (const key of schema.required ?? []) {
      if (!(key in value)) errors.push(`${path}: brak pola „${key}”`);
    }
    for (const [key, item] of Object.entries(value)) {
      const known = schema.properties?.[key];
      if (known) errors.push(...schemaErrors(root, known, item, `${path}.${key}`));
      else if (schema.additionalProperties === false) errors.push(`${path}: obce pole „${key}”`);
      else if (schema.additionalProperties && schema.additionalProperties !== true) {
        errors.push(...schemaErrors(root, schema.additionalProperties, item, `${path}.${key}`));
      }
    }
  }
  return errors;
}

/** Ślad poleceń okna: opakowanie `__TAURI_INTERNALS__.invoke` przez CDP, zapis nazw do pliku. */
export async function traceIpc(page: Page, outDir: string): Promise<void> {
  mkdirSync(outDir, { recursive: true });
  const file = join(outDir, 'ipc-seen.txt');
  const binding = `__alfaE2eIpc${Date.now()}`;
  await page.exposeFunction(binding, (command: string) => {
    appendFileSync(file, `${command}\n`);
  });
  await page.addInitScript((name: string) => {
    type Invoke = (command: string, ...rest: unknown[]) => unknown;
    interface Internals {
      invoke?: Invoke;
      __alfaTraced?: boolean;
    }
    const host = window as unknown as Record<string, unknown>;
    const wrap = (internals: Internals | undefined): void => {
      if (!internals?.invoke || internals.__alfaTraced) return;
      const original = internals.invoke.bind(internals);
      internals.__alfaTraced = true;
      internals.invoke = (command, ...rest) => {
        // Wtyczki Tauri (`plugin:…`) nie są poleceniami rdzenia Alfy.
        if (!command.startsWith('plugin:'))
          (host[name] as ((c: string) => void) | undefined)?.(command);
        return original(command, ...rest);
      };
    };
    // Tauri wstawia obiekt przed skryptami strony albo tuż po nich — opakowujemy w obu przypadkach.
    wrap(host['__TAURI_INTERNALS__'] as Internals | undefined);
    const timer = setInterval(() => wrap(host['__TAURI_INTERNALS__'] as Internals | undefined), 5);
    setTimeout(() => clearInterval(timer), 10_000);
  }, binding);
}
