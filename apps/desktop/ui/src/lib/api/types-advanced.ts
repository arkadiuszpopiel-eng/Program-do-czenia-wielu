// Ustawienia → Zaawansowane (komendy `config_raw_*`): surowe pliki konfiguracji.

export interface RawConfig {
  /** `shared.toml` (wspólny dla maszyn). */
  readonly shared: string;
  /** Nakładka tej maszyny. */
  readonly machine: string;
  readonly shared_path: string;
  readonly machine_path: string;
  /** Problemy z ostatniego wczytania plików. */
  readonly problems: readonly string[];
}
