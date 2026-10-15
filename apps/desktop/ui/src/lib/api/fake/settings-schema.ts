// Drzewo ustawień (PLAN §15, docs/UI.md §12) w atrapie. W buildzie z Tauri schemat składa rdzeń
// z manifestów modułów (każdy moduł dostarcza własną stronę) — komenda `settings_schema`.
import type { SettingValue, SettingsPageDef } from '../types-system';
import { SETTINGS_PART_A } from './settings-part-a';
import { SETTINGS_PART_B } from './settings-part-b';
import { L, page, select, toggle } from './settings-helpers';

/** Strony dodane po podziale na części (każda część jest u limitu długości pliku). */
const SETTINGS_LATER: readonly SettingsPageDef[] = [
  page('mcp', L('Serwery MCP', 'MCP servers'), 1, [], { custom: 'mcp' }),
  page('diag', L('Diagnostyka komputera', 'Computer diagnostics'), 1, [], { custom: 'diag' }),
  page(
    'advanced',
    L('Zaawansowane', 'Advanced'),
    1,
    [
      toggle(
        'experimental.diag_daily',
        L('Codzienny zapis parametrów komputera', 'Daily computer parameter record'),
        L(
          'Raz na dobę Alfa zapisuje parametry komputera i zdarzenia awarii do historii diagnostyki (kilka sekund, tylko odczyt). Zmiana po ponownym uruchomieniu.',
          'Once a day Alfa records computer parameters and failure events to the diagnostics history (a few seconds, read-only). Takes effect after a restart.',
        ),
        true,
        'machine',
      ),
    ],
    { custom: 'advanced' },
  ),
  page(
    'commands',
    L('Polecenia bez modelu', 'Commands without a model'),
    1,
    [
      toggle(
        'sessions.local_commands',
        L('Odpowiadaj bez modelu', 'Answer without a model'),
        L(
          'Rachunki, literowanie, losowanie, data i godzina — Alfa odpowiada od razu, bez modelu i bez kosztu, także offline.',
          'Arithmetic, spelling, random draws, date and time — Alfa answers at once, without a model or cost, offline too.',
        ),
        true,
      ),
    ],
    { custom: 'commands' },
  ),
  page('templates', L('Szablony promptów', 'Prompt templates'), 1, [], { custom: 'templates' }),
  page('help', L('Instrukcja', 'User guide'), 1, [], { custom: 'help' }),
];

/** Ustawienia dopisane do stron z części (części są u limitu długości pliku). */
const EXTRA: Readonly<Record<string, SettingsPageDef['settings']>> = {
  general: [
    select(
      'chat.reply_style',
      L('Styl odpowiedzi', 'Reply style'),
      L(
        'Jak agentki mają odpowiadać w rozmowie. „Automatycznie” — tak, jak każda agentka ma w swojej personie.',
        'How the agents reply in conversation. “Automatic” — as each agent’s persona says.',
      ),
      [
        ['auto', L('Automatycznie', 'Automatic')],
        ['concise', L('Zwięźle', 'Concise')],
        ['detailed', L('Szczegółowo', 'Detailed')],
        ['simple', L('Prostym językiem', 'Plain language')],
        ['formal', L('Formalnie', 'Formal')],
      ],
      'auto',
    ),
    select(
      'chat.temperature',
      L('Swoboda odpowiedzi', 'Reply freedom'),
      L(
        'Precyzyjnie — przewidywalne, powtarzalne odpowiedzi; twórczo — więcej pomysłów i różnorodności. Modele, które tego nie obsługują, odpowiadają po swojemu.',
        'Precise — predictable, repeatable replies; creative — more ideas and variety. Models that do not support it answer their own way.',
      ),
      [
        ['auto', L('Automatycznie (domyślna modelu)', 'Automatic (model default)')],
        ['precise', L('Precyzyjnie', 'Precise')],
        ['balanced', L('Zrównoważenie', 'Balanced')],
        ['creative', L('Twórczo', 'Creative')],
      ],
      'auto',
    ),
  ],
};

export const SETTINGS_SCHEMA: readonly SettingsPageDef[] = [
  ...SETTINGS_PART_A,
  ...SETTINGS_PART_B,
  ...SETTINGS_LATER,
].map((p) => (EXTRA[p.id] ? { ...p, settings: [...p.settings, ...(EXTRA[p.id] ?? [])] } : p));

export function defaultValues(): Record<string, SettingValue> {
  const out: Record<string, SettingValue> = {};
  for (const p of SETTINGS_SCHEMA) for (const s of p.settings) out[s.key] = s.default;
  return out;
}
