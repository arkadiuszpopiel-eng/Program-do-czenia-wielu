// Drzewo ustawień (PLAN §15, docs/UI.md §12) w atrapie. W buildzie z Tauri schemat składa rdzeń
// z manifestów modułów (każdy moduł dostarcza własną stronę) — komenda `settings_schema`.
import type { SettingValue, SettingsPageDef } from '../types-system';
import { SETTINGS_PART_A } from './settings-part-a';
import { SETTINGS_PART_B } from './settings-part-b';
import { L, page } from './settings-helpers';

/** Strony dodane po podziale na części (każda część jest u limitu długości pliku). */
const SETTINGS_LATER: readonly SettingsPageDef[] = [
  page('mcp', L('Serwery MCP', 'MCP servers'), 1, [], { custom: 'mcp' }),
  page('diag', L('Diagnostyka komputera', 'Computer diagnostics'), 1, [], { custom: 'diag' }),
];

export const SETTINGS_SCHEMA: readonly SettingsPageDef[] = [
  ...SETTINGS_PART_A,
  ...SETTINGS_PART_B,
  ...SETTINGS_LATER,
];

export function defaultValues(): Record<string, SettingValue> {
  const out: Record<string, SettingValue> = {};
  for (const p of SETTINGS_SCHEMA) for (const s of p.settings) out[s.key] = s.default;
  return out;
}
