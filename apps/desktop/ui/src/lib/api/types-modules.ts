// Ustawienia → Moduły (komendy `modules_*`): stan modułów z manifestów i rejestru; wyłączanie
// modułów opcjonalnych działa od następnego uruchomienia.

export interface ModuleRow {
  readonly id: string;
  readonly version: string;
  readonly kind: string;
  readonly lifecycle: string;
  readonly isolation: string;
  /** `disabled`, `unloaded`, `loading`, `ready`, `degraded`, `failed`. */
  readonly state: string;
  readonly detail: string | null;
  readonly ram_mb: number;
  readonly cpu_pct: number;
  readonly vram_mb: number | null;
  readonly capabilities: readonly string[];
  readonly provides: readonly string[];
  readonly requires: readonly string[];
  /** Moduł Jądra — bez wyłączania. */
  readonly protected: boolean;
  readonly can_disable: boolean;
  readonly disabled: boolean;
  readonly skipped: boolean;
}

export interface ModulesView {
  readonly modules: readonly ModuleRow[];
  readonly restart_needed: boolean;
  readonly ram_total_mb: number;
}
