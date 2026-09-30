// DTO: Hub kont i kluczy (accounts-hub), import/eksport `.alfa` (transfer), uprawnienia, urządzenia.
import type { AgentId } from '@alfa/ui-kit';
import type { AutonomyLevel, Iso8601, LocalizedText, Money } from './types';

// ── Konta i klucze (PLAN §5.6) ──────────────────────────────────────────────────────────────────

export type ProviderKind = 'chat' | 'stt' | 'tts' | 'multi';
export type ComplianceStatus = 'green' | 'gray' | 'forbidden' | 'unverified';

/** Wpis katalogu dostawców (`providers-catalog/*.toml`). */
export interface ProviderInfo {
  readonly id: string;
  readonly display_name: string;
  readonly kind: ProviderKind;
  readonly auth: 'api_key' | 'oauth_cli' | 'none';
  readonly compat: 'openai' | 'anthropic' | 'native';
  readonly privacy_tag: string;
  readonly jurisdiction: string;
  readonly compliance_status: ComplianceStatus;
  readonly terms_url: string | null;
  /** Własny endpoint: kreator pyta o adres bazowy. */
  readonly needs_base_url: boolean;
}

export type AccountState =
  'unconfigured' | 'testing' | 'ok' | 'invalid' | 'rate_limited' | 'disabled';

export interface ModelInfo {
  readonly id: string;
  readonly name: string;
  readonly kinds: readonly ('chat' | 'stt' | 'tts' | 'embeddings' | 'vision')[];
  readonly context_tokens: number | null;
}

export interface AccountAssignment {
  /** Klasy zadań routera (np. `chat`, `code`, `background`). */
  readonly task_classes: readonly string[];
  readonly agents: readonly AgentId[];
  readonly voice_stt: boolean;
  readonly voice_tts: boolean;
}

/** Konto u dostawcy. Sekret nigdy nie wraca do UI — tylko informacja, że jest w Credential Managerze. */
export interface Account {
  readonly id: string;
  readonly provider_id: string;
  readonly label: string;
  readonly state: AccountState;
  readonly key_stored: boolean;
  readonly models: readonly ModelInfo[];
  readonly assignments: AccountAssignment;
  readonly cost_limit: { readonly enabled: boolean; readonly monthly: Money };
  readonly last_tested_at: Iso8601 | null;
}

/** Dane kreatora. `secret` trafia jednorazowo do backendu (Credential Manager); UI go nie przechowuje. */
export interface AddAccountInput {
  readonly provider_id: string;
  readonly label: string;
  readonly secret: string;
  readonly base_url: string | null;
}

export interface TestReport {
  readonly ok: boolean;
  readonly latency_ms: number | null;
  readonly models: readonly ModelInfo[];
  readonly error: string | null;
}

// ── Import / eksport `.alfa` (PLAN §15.1, docs/formats/alfa-package.md) ─────────────────────────

export interface ExportScope {
  readonly config_common: boolean;
  readonly personas: boolean;
  readonly casts: boolean;
  readonly sessions: readonly string[];
  /** Pozycje z F7 (pamięć, umiejętności, artefakty, logi, nakładka maszyny) — w F1 zawsze `false`. */
  readonly artifacts: boolean;
  readonly logs: boolean;
  readonly config_machine: boolean;
}

export interface ExportRequest {
  readonly scope: ExportScope;
  /** Opcjonalne szyfrowanie całej paczki hasłem. Klucze API nigdy nie wchodzą do eksportu. */
  readonly password: string | null;
}

export type ExportResult =
  | { readonly status: 'cancelled' }
  | {
      readonly status: 'saved';
      readonly path: string;
      readonly files: number;
      readonly bytes: number;
    };

export type ItemDiff = 'new' | 'same' | 'changed' | 'collision';

export interface DryRunItem {
  readonly key: string;
  readonly kind: 'session' | 'config' | 'persona' | 'cast';
  readonly label: string;
  readonly diff: ItemDiff;
}

export interface PackageManifestSummary {
  readonly schema_version: string;
  readonly app_version: string;
  readonly created_at: Iso8601;
  readonly source_machine: string;
  readonly encrypted: boolean;
}

export type InspectResult =
  | { readonly status: 'cancelled' }
  | { readonly status: 'needs_password'; readonly path: string }
  | {
      readonly status: 'inspected';
      readonly path: string;
      readonly manifest: PackageManifestSummary;
      readonly items: readonly DryRunItem[];
      readonly warnings: readonly string[];
      readonly migrations: readonly string[];
    };

export type ImportMode = 'add' | 'merge' | 'replace';
export type CollisionResolution = 'keep_local' | 'take_imported' | 'keep_both';

export interface ImportRequest {
  readonly path: string;
  readonly mode: ImportMode;
  readonly resolutions: Readonly<Record<string, CollisionResolution>>;
  readonly password: string | null;
}

export interface ImportResult {
  /** Automatyczny snapshot przed importem → jednoklikowy rollback. */
  readonly snapshot_id: string;
  readonly imported: number;
  readonly skipped: number;
}

// ── Uprawnienia (PLAN §8.3) ──────────────────────────────────────────────────────────────────────

export interface PermissionsState {
  readonly global: AutonomyLevel;
  readonly session: AutonomyLevel | null;
  readonly hello_enabled: boolean;
}

/** Zmiana poziomu to intencja: Broker otwiera swoje okno i tam ją potwierdzasz. */
export interface BrokerIntentResult {
  readonly status: 'opened_broker';
  readonly request_id: string;
}

// ── Urządzenia (device-profile) ─────────────────────────────────────────────────────────────────

export type HwClass = 'baseline' | 'standard_amd' | 'laptop_cuda' | 'strong' | 'unknown';
export type VoiceProfileId = 'A' | 'B' | 'C' | 'D';

export interface GpuInfo {
  readonly vendor: string;
  readonly model: string;
  readonly vram_mb: number;
  readonly backends: readonly string[];
}

export interface DeviceProfile {
  readonly machine: {
    readonly id: string;
    readonly name: string;
    readonly os: string;
    readonly cpu: { readonly model: string; readonly cores: number; readonly threads: number };
    readonly ram_mb: number;
    readonly gpus: readonly GpuInfo[];
    readonly npu: string | null;
    readonly battery: { readonly percent: number; readonly on_ac: boolean } | null;
  };
  readonly recommendation: {
    readonly hw_class: HwClass;
    readonly voice_profile: VoiceProfileId;
    readonly llm_backend: string;
    readonly tradeoffs: readonly LocalizedText[];
  };
  readonly measured_at: Iso8601;
}

export interface AudioDevice {
  readonly id: string;
  readonly name: string;
  readonly default: boolean;
}
