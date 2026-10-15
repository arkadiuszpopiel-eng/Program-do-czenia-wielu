// Diagnostyka komputera (Ustawienia → Diagnostyka komputera; komendy `diag_*`). Typy kontraktu
// `pc-diag` (Rust) — zgodność pilnuje `generated/compat.ts`. Pola bez odczytu (brak uprawnień,
// brak czujnika) są `null`, nie zgadywane.

export interface MachineDiag {
  readonly os: string;
  readonly os_version: string;
  readonly model: string | null;
  readonly bios: string | null;
  readonly uptime_s: number;
}

export interface CpuDiag {
  readonly name: string;
  readonly cores: number;
  readonly threads: number;
  readonly base_mhz: number | null;
  readonly current_mhz: number | null;
  readonly load_pct: number | null;
  readonly l2_kb: number | null;
  readonly l3_kb: number | null;
  readonly virtualization: boolean | null;
}

export interface MemModule {
  readonly slot: string | null;
  readonly capacity_mb: number;
  readonly speed_mhz: number | null;
  readonly manufacturer: string | null;
  readonly part: string | null;
}

export interface MemDiag {
  readonly total_mb: number;
  readonly available_mb: number;
  readonly pagefile_mb: number | null;
  readonly pagefile_used_mb: number | null;
  readonly modules: readonly MemModule[];
}

export type MediaKind = 'ssd' | 'hdd' | 'unknown';
export type Health = 'healthy' | 'warning' | 'unhealthy' | 'unknown';

export interface VolumeDiag {
  readonly letter: string;
  readonly label: string | null;
  readonly file_system: string | null;
  readonly size_mb: number;
  readonly free_mb: number;
  readonly system: boolean;
}

export interface DiskDiag {
  readonly number: number | null;
  readonly model: string;
  readonly serial_tail: string | null;
  readonly media: MediaKind;
  readonly bus: string | null;
  readonly size_mb: number;
  readonly health: Health;
  readonly operational: string | null;
  readonly wear_pct: number | null;
  readonly temperature_c: number | null;
  readonly power_on_hours: number | null;
  readonly read_errors: number | null;
  readonly write_errors: number | null;
  readonly removable: boolean;
  readonly volumes: readonly string[];
  readonly event_errors: number;
}

export interface GpuDiag {
  readonly name: string;
  readonly driver_version: string | null;
  readonly driver_date: string | null;
  readonly vram_mb: number | null;
  readonly status: string | null;
}

export interface NetDiag {
  readonly name: string;
  readonly description: string | null;
  readonly up: boolean;
  readonly link_mbps: number | null;
  readonly kind: string;
}

export interface BatteryDiag {
  readonly charge_pct: number | null;
  readonly on_ac: boolean | null;
  readonly design_mwh: number | null;
  readonly full_mwh: number | null;
  readonly cycles: number | null;
}

export interface ThermalDiag {
  readonly name: string;
  readonly celsius: number;
}

export type EventKind =
  'unexpected_shutdown' | 'bugcheck' | 'hardware' | 'disk' | 'usb' | 'app_crash' | 'other';

export interface EventLine {
  readonly at: string;
  readonly source: string;
  readonly id: number;
  readonly kind: EventKind;
  readonly message: string;
}

export interface EventCount {
  readonly kind: EventKind;
  readonly count: number;
}

export interface EventSummary {
  readonly window_days: number;
  readonly counts: readonly EventCount[];
  readonly recent: readonly EventLine[];
}

export interface Snapshot {
  readonly at: string;
  readonly machine: MachineDiag;
  readonly cpu: CpuDiag;
  readonly memory: MemDiag;
  readonly disks: readonly DiskDiag[];
  readonly volumes: readonly VolumeDiag[];
  readonly gpus: readonly GpuDiag[];
  readonly network: readonly NetDiag[];
  readonly battery: BatteryDiag | null;
  readonly thermal: readonly ThermalDiag[];
  readonly events: EventSummary;
  readonly gaps: readonly string[];
}

export type CheckKind =
  | { readonly test: 'cpu_single' }
  | { readonly test: 'cpu_multi' }
  | { readonly test: 'memory' }
  | { readonly test: 'disk'; readonly target: string }
  | { readonly test: 'network' }
  | { readonly test: 'integrity' };

export type Verdict = 'pass' | 'warn' | 'fail' | 'skipped';

export interface Metric {
  readonly key: string;
  readonly label: string;
  readonly value: number;
  readonly unit: string;
  readonly higher_is_better: boolean;
}

export interface CheckResult {
  readonly kind: CheckKind;
  readonly verdict: Verdict;
  readonly metrics: readonly Metric[];
  readonly notes: readonly string[];
  readonly duration_ms: number;
}

export interface CheckRun {
  readonly at: string;
  readonly results: readonly CheckResult[];
  readonly overall: Verdict;
}

export interface DiagProgress {
  readonly index: number;
  readonly total: number;
  readonly current: string;
  readonly done: readonly CheckResult[];
}

export interface DiagView {
  readonly latest: Snapshot | null;
  readonly integrity: CheckResult | null;
  readonly last_run: CheckRun | null;
  readonly available: readonly string[];
  readonly running: DiagProgress | null;
  readonly snapshots: number;
  readonly runs: number;
  readonly recording: boolean;
}
