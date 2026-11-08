// Atrapa: diagnostyka komputera. Deterministyczna: laptop z dyskiem NVMe (C:) i dyskiem na mostku
// USB (E:) z błędami w dzienniku, niebieski ekran 0x116, wynik ostrzegawczy. Testy kończą się od
// razu z ustalonymi miarami; drugi przebieg tego samego testu porównuje się z pierwszym jak w
// rdzeniu (spadek „większa lepsza” ≥ 30% — ostrzeżenie).
import type { DiagApi } from '../client-diag';
import type {
  CheckKind,
  CheckResult,
  CheckRun,
  DiagView,
  Metric,
  Snapshot,
  Verdict,
} from '../types-diag';
import type { FakeCore } from './core';

const metric = (
  key: string,
  label: string,
  value: number,
  unit: string,
  better = true,
): Metric => ({
  key,
  label,
  value,
  unit,
  higher_is_better: better,
});

export function sampleSnapshot(at: string): Snapshot {
  return {
    at,
    machine: {
      os: 'Windows 11 Pro',
      os_version: '10.0.26200',
      model: 'Laptop 15',
      bios: '1.20',
      uptime_s: 3 * 3600,
    },
    cpu: {
      name: 'Procesor 10C/16T',
      cores: 10,
      threads: 16,
      base_mhz: 2400,
      current_mhz: 2400,
      load_pct: 12,
      l2_kb: 9728,
      l3_kb: 24576,
      virtualization: true,
    },
    memory: {
      total_mb: 16_384,
      available_mb: 5_120,
      pagefile_mb: 28_672,
      pagefile_used_mb: 700,
      modules: [
        {
          slot: 'DIMM 0',
          capacity_mb: 8192,
          speed_mhz: 4800,
          manufacturer: 'Producent',
          part: 'M-8G',
        },
        {
          slot: 'DIMM 1',
          capacity_mb: 8192,
          speed_mhz: 4800,
          manufacturer: 'Producent',
          part: 'M-8G',
        },
      ],
    },
    disks: [
      {
        number: 1,
        model: 'NVMe 512GB',
        serial_tail: '…4F2A',
        media: 'ssd',
        bus: 'NVMe',
        size_mb: 488_386,
        health: 'healthy',
        operational: 'OK',
        wear_pct: null,
        temperature_c: null,
        power_on_hours: null,
        read_errors: null,
        write_errors: null,
        removable: false,
        volumes: ['C:'],
        event_errors: 0,
      },
      {
        number: 2,
        model: 'Mostek USB NVMe',
        serial_tail: null,
        media: 'ssd',
        bus: 'USB',
        size_mb: 488_386,
        health: 'healthy',
        operational: 'OK',
        wear_pct: null,
        temperature_c: null,
        power_on_hours: null,
        read_errors: null,
        write_errors: null,
        removable: true,
        volumes: ['E:'],
        event_errors: 200,
      },
    ],
    volumes: [
      {
        letter: 'C:',
        label: 'OS',
        file_system: 'NTFS',
        size_mb: 487_500,
        free_mb: 103_500,
        system: true,
      },
      {
        letter: 'E:',
        label: 'Dane',
        file_system: 'NTFS',
        size_mb: 488_300,
        free_mb: 68_400,
        system: false,
      },
    ],
    gpus: [
      {
        name: 'Karta graficzna 6 GB',
        driver_version: '32.0.16.1692',
        driver_date: '2026-09-04',
        vram_mb: 6144,
        status: 'OK',
      },
      {
        name: 'Grafika zintegrowana',
        driver_version: '32.0.101.7082',
        driver_date: '2025-12-01',
        vram_mb: 2048,
        status: 'OK',
      },
    ],
    network: [
      { name: 'Wi-Fi', description: 'Karta Wi-Fi', up: true, link_mbps: 1201, kind: 'wifi' },
      {
        name: 'Ethernet',
        description: 'Karta Ethernet',
        up: false,
        link_mbps: null,
        kind: 'ethernet',
      },
    ],
    battery: { charge_pct: 100, on_ac: true, design_mwh: null, full_mwh: null, cycles: null },
    thermal: [{ name: 'Strefa CPU', celsius: 71 }],
    events: {
      window_days: 7,
      counts: [
        { kind: 'unexpected_shutdown', count: 4 },
        { kind: 'bugcheck', count: 2 },
        { kind: 'disk', count: 275 },
        { kind: 'usb', count: 5 },
      ],
      recent: [
        {
          at: '2026-10-15T09:59:59Z',
          source: 'Microsoft-Windows-WER-SystemErrorReporting',
          id: 1001,
          kind: 'bugcheck',
          message: 'Ponowny rozruch po błędzie: 0x00000116 (…)',
        },
        {
          at: '2026-10-15T09:05:08Z',
          source: 'disk',
          id: 51,
          kind: 'disk',
          message: 'Błąd urządzenia \\Device\\Harddisk2\\DR19 podczas stronicowania. (×9)',
        },
      ],
    },
    gaps: [
      'liczniki SMART dysków (zużycie, godziny pracy, błędy) — wymagają uprawnień administratora',
    ],
  };
}

const INTEGRITY_NOTES = [
  '2 niebieskich ekranów w ostatnich 7 dniach.',
  '4 nieoczekiwanych wyłączeń w ostatnich 7 dniach.',
  '275 błędów dysku lub systemu plików w ostatnich 7 dniach.',
  'Niebieski ekran 0x116 VIDEO_TDR_FAILURE — karta graficzna przestała odpowiadać — sterownik, przegrzanie albo zasilanie karty.',
  'Dysk Mostek USB NVMe (E:): 200 błędów w dzienniku — sprawdź kabel, mostek USB i kopię danych.',
];

export const kindOf = (key: string): CheckKind | null => {
  if (key.startsWith('disk:')) return { test: 'disk', target: key.slice(5).toUpperCase() };
  const known = ['cpu_single', 'cpu_multi', 'memory', 'network', 'integrity'] as const;
  const k = known.find((x) => x === key);
  return k ? { test: k } : null;
};

function measure(key: string, kind: CheckKind): CheckResult {
  const r = (verdict: Verdict, metrics: Metric[], notes: string[]): CheckResult => ({
    kind,
    verdict,
    metrics,
    notes,
    duration_ms: 1500,
  });
  switch (kind.test) {
    case 'integrity':
      return r(
        'fail',
        [metric('disk_events', 'Błędy dysku (dni)', 275, 'szt.', false)],
        INTEGRITY_NOTES,
      );
    case 'cpu_single':
      return r('pass', [metric('mops', 'Wynik (jeden wątek)', 410, 'mln op/s')], []);
    case 'cpu_multi':
      return r(
        'pass',
        [
          metric('mops', 'Wynik (wszystkie wątki)', 3900, 'mln op/s'),
          metric('scaling', 'Skalowanie', 9.5, '×'),
        ],
        [],
      );
    case 'memory':
      return r(
        'pass',
        [
          metric('tested_mb', 'Sprawdzono', 512, 'MB'),
          metric('errors', 'Błędy wzorca', 0, 'szt.', false),
          metric('copy_gbps', 'Kopiowanie', 11.2, 'GB/s'),
        ],
        ['Wzorzec zapisany i odczytany bez błędów.'],
      );
    case 'network':
      return r(
        'pass',
        [
          metric('dns_ms', 'Rozwiązywanie nazw', 18, 'ms', false),
          metric('connect_ms', 'Nawiązanie połączenia', 24, 'ms', false),
        ],
        ['Połączenie z internetem działa.'],
      );
    case 'disk': {
      const usb = kind.target === 'E:';
      return r(
        usb ? 'warn' : 'pass',
        [
          metric('seq_write_mbps', 'Zapis sekwencyjny', usb ? 140 : 1450, 'MB/s'),
          metric('seq_read_mbps', 'Odczyt sekwencyjny', usb ? 320 : 2100, 'MB/s'),
          metric('verify_errors', 'Bloki z błędną sumą', 0, 'szt.', false),
          metric('rand_read_ms', 'Odczyt losowy 4 KB (średnio)', usb ? 0.9 : 0.1, 'ms', false),
        ],
        usb ? [`Zapis 140 MB/s — wolniej niż typowo (próg 150) [${key}].`] : [],
      );
    }
  }
}

function overall(results: readonly CheckResult[]): Verdict {
  const order: Verdict[] = ['pass', 'warn', 'fail'];
  const real = results.map((x) => x.verdict).filter((v) => v !== 'skipped');
  if (!real.length) return 'skipped';
  return order[Math.max(...real.map((v) => order.indexOf(v)))] ?? 'pass';
}

export class FakeDiag {
  private latest: Snapshot | null = null;
  private history: CheckRun[] = [];

  constructor(private readonly core: FakeCore) {
    this.latest = sampleSnapshot(core.isoNow());
  }

  private integrity(): CheckResult | null {
    return this.latest ? measure('integrity', { test: 'integrity' }) : null;
  }

  private view(): DiagView {
    const keys = ['integrity', 'cpu_single', 'cpu_multi', 'memory', 'network'];
    return {
      latest: this.latest,
      integrity: this.integrity(),
      last_run: this.history.at(-1) ?? null,
      available: [...keys, ...(this.latest?.volumes.map((v) => `disk:${v.letter}`) ?? [])],
      running: null,
      snapshots: this.latest ? 1 : 0,
      runs: this.history.length,
      recording: false,
    };
  }

  private run(tests: readonly string[]): CheckRun {
    if (!tests.length) throw new Error('Wybierz od 1 do 32 testów.');
    const results = tests.map((key) => {
      const kind = kindOf(key);
      if (!kind) throw new Error(`Nieznany test \`${key}\`.`);
      return measure(key, kind);
    });
    const run: CheckRun = { at: this.core.isoNow(), results, overall: overall(results) };
    this.history = [...this.history, run];
    return run;
  }

  api(): DiagApi {
    const core = this.core;
    return {
      state: () => core.reply(this.view()),
      snapshot: async () => {
        this.latest = sampleSnapshot(core.isoNow());
        return core.reply(this.view());
      },
      run: async (tests) => core.reply(this.run(tests)),
      cancel: () => core.reply(undefined),
      runs: async (limit) => core.reply([...this.history].reverse().slice(0, limit)),
      export: () =>
        core.reply('C:\\Users\\Ty\\Alfa\\Diagnostyka\\diagnostyka-2026-10-15-120000.md'),
    };
  }
}
