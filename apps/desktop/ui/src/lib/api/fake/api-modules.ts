// Atrapa: Ustawienia → Moduły. Kilka modułów w różnych stanach (gotowy, zwolniony, z błędem),
// moduły Jądra chronione, pięć opcjonalnych do wyłączenia (od „następnego uruchomienia” —
// atrapa zapamiętuje ustawienie, a `skipped` zostaje z „uruchomienia”).
import type { ModulesApi } from '../client-modules';
import type { ModuleRow, ModulesView } from '../types-modules';
import type { FakeCore } from './core';

const OPTIONAL = [
  'providers-local',
  'voice-audio',
  'voice-tts',
  'memory-consolidation',
  'transfer',
];
const PROTECTED = new Set([
  'safety-broker',
  'broker-ui',
  'watchdog',
  'updater',
  'risk-classifier',
  'compliance',
]);

const row = (
  id: string,
  kind: string,
  lifecycle: string,
  state: string,
  ram_mb: number,
  extra: Partial<ModuleRow> = {},
): ModuleRow => ({
  id,
  version: '0.0.59',
  kind,
  lifecycle,
  isolation: 'inproc',
  state,
  detail: null,
  ram_mb,
  cpu_pct: 5,
  vram_mb: null,
  capabilities: [],
  provides: [`${id}-contract@1`],
  requires: [],
  protected: PROTECTED.has(id) || id.startsWith('core-'),
  can_disable: OPTIONAL.includes(id),
  disabled: false,
  skipped: false,
  ...extra,
});

function seed(): ModuleRow[] {
  return [
    row('core-config', 'service', 'always', 'ready', 8),
    row('safety-broker', 'service', 'always', 'ready', 32, {
      isolation: 'process',
      capabilities: ['fs.read(path)', 'fs.write(path)', 'shell.exec(path)'],
    }),
    row('sessions', 'service', 'always', 'ready', 24),
    row('providers-local', 'provider', 'lazy', 'unloaded', 6144, { vram_mb: 6144 }),
    row('voice-audio', 'voice-engine', 'lazy', 'ready', 64),
    row('voice-tts', 'voice-engine', 'lazy', 'unloaded', 300),
    row('memory-consolidation', 'service', 'on-demand', 'unloaded', 64),
    row('transfer', 'service', 'lazy', 'failed', 32, { detail: 'brak miejsca na dysku' }),
    row('tools-browser', 'tool', 'lazy', 'unloaded', 16, { capabilities: ['net.egress(host)'] }),
  ];
}

export class FakeModules {
  private rows: ModuleRow[] = seed();

  constructor(private readonly core: FakeCore) {}

  private view(): ModulesView {
    const modules = [...this.rows].sort((a, b) => a.id.localeCompare(b.id));
    return {
      modules,
      restart_needed: modules.some((m) => m.disabled !== m.skipped),
      ram_total_mb: modules.filter((m) => !m.disabled).reduce((s, m) => s + m.ram_mb, 0),
    };
  }

  private set(id: string, enabled: boolean): ModulesView {
    const target = this.rows.find((m) => m.id === id);
    if (!target?.can_disable) {
      throw new Error(`Modułu \`${id}\` nie można wyłączyć — jest niezbędny albo należy do Jądra.`);
    }
    this.rows = this.rows.map((m) => (m.id === id ? { ...m, disabled: !enabled } : m));
    return this.view();
  }

  api(): ModulesApi {
    return {
      list: () => this.core.reply(this.view()),
      setEnabled: async (id, enabled) => this.core.reply(this.set(id, enabled)),
    };
  }
}
