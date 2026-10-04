// Wtyczki Wasm (komendy `plugins_*`, Ustawienia → „Wtyczki”) — kształt 1:1
// z `crates/app-api/src/dto/plugins.rs` (pola snake_case). Wersje jako tekst semver.

export type PluginStateView = 'proposed' | 'installed' | 'disabled' | 'rejected' | 'superseded';

/** Pochodzenie: plik właściciela, Ulepszacz (R2), własna paczka `.alfa`, paczka z zewnątrz. */
export type PluginOrigin = 'user' | 'improver' | 'import' | 'external';

/** Zadeklarowana zdolność (`fs.read`, `fs.write`, `net.egress`) z zakresem. */
export interface PluginCapabilityView {
  readonly family: string;
  readonly scope: string;
}

/** Limity piaskownicy jednego wywołania. */
export interface PluginLimitsView {
  readonly memory_mib: number;
  readonly fuel_per_call: number;
  readonly wall_ms: number;
  readonly max_input_bytes: number;
  readonly max_output_bytes: number;
  readonly max_host_calls: number;
}

/** Narzędzie widziane przez agentki (`plugin_<nazwa>`). */
export interface PluginToolView {
  readonly name: string;
  readonly title: string;
  readonly description: string;
  readonly mutating: boolean;
}

/** Zmiana pierścienia R2 (klucz `plugins.<id>.version` = hash przejrzanej wersji). */
export interface PluginR2View {
  readonly key: string;
  readonly value: string;
  readonly from_version: string | null;
  readonly added_capabilities: readonly string[];
}

/** Wersja wtyczki w bibliotece (karta zatwierdzenia). */
export interface PluginInfo {
  readonly id: string;
  readonly version: string;
  readonly author: string;
  readonly description: string;
  readonly state: PluginStateView;
  readonly origin: PluginOrigin;
  readonly wasm_sha256: string;
  /** Hash przejrzanej wersji (obejmuje hash modułu) — wysyłany przy zatwierdzeniu. */
  readonly review_hash: string;
  readonly capabilities: readonly PluginCapabilityView[];
  readonly limits: PluginLimitsView;
  readonly tools: readonly PluginToolView[];
  /** Zapis plików albo sieć — narzędzia nieodwracalne. */
  readonly side_effects: boolean;
  readonly proposed_at: string;
  readonly decided_at: string | null;
  /** Propozycja R2 (tylko dla wersji czekającej na zatwierdzenie). */
  readonly r2: PluginR2View | null;
}

export type PluginProblemKind = 'trapped' | 'load_failed';

/** Ostatni problem wtyczki (bez treści wejścia/wyjścia). */
export interface PluginProblem {
  readonly plugin: string;
  readonly version: string;
  readonly kind: PluginProblemKind;
  readonly detail: string;
  readonly at: string;
}

export interface PluginsView {
  readonly available: boolean;
  readonly unavailable_reason: string | null;
  readonly plugins: readonly PluginInfo[];
  readonly problems: readonly PluginProblem[];
}

/** Wynik kontroli modułu przed propozycją. */
export interface PluginInspection {
  readonly ok: boolean;
  readonly wasm_sha256: string;
  readonly bytes: number;
  readonly error: string | null;
}
