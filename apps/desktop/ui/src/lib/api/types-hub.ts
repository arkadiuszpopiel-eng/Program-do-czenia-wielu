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
  /** Przeznaczenie rozpoznane przez rdzeń po nazwie modelu. */
  readonly purpose: ModelPurpose;
  /** Wielkość modelu w swojej rodzinie. */
  readonly tier: ModelTier;
  /** Przypięta migawka z datą w nazwie (np. `…-2024-08-06`). */
  readonly snapshot: boolean;
}

export type ModelPurpose =
  | 'chat'
  | 'reasoning'
  | 'embedding'
  | 'speech'
  | 'transcription'
  | 'realtime'
  | 'image'
  | 'video'
  | 'moderation'
  | 'other';
export type ModelTier = 'standard' | 'small' | 'tiny' | 'premium';

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
  /** Kiedy zapisano obecny klucz (dodanie konta albo ostatnia wymiana). */
  readonly key_saved_at?: Iso8601 | null;
  /** Wynik ostatniego testu — lista kont pokazuje powód, nie tylko „klucz odrzucony”. */
  readonly last_test?: AccountLastTest | null;
  /** Kolejki modeli: klasa zadań → modele w kolejności prób (brak klasy = automatycznie). */
  readonly queues?: Readonly<Record<string, readonly string[]>>;
  /** Model, którego Alfa użyje do rozmowy bez kolejki. */
  readonly auto_model?: string | null;
  /** Konto firmowe: organizacja i projekt OpenAI albo wersja API i wdrożenia Azure OpenAI. */
  readonly enterprise?: EnterpriseSettings;
  /** Adres konta (Azure: zasób), gdy rdzeń go podaje; brak — adres domyślny dostawcy. */
  readonly base_url?: string | null;
}

/** Wdrożenie modelu w zasobie Azure OpenAI: Alfa wybiera model, żądanie idzie na wdrożenie. */
export interface AzureDeployment {
  /** Nazwa wdrożenia w zasobie (część adresu `/openai/deployments/{name}/…`). */
  readonly name: string;
  /** Model udostępniany przez wdrożenie (np. `gpt-4o`); brak — model nazywa się jak wdrożenie. */
  readonly model?: string | null;
}

/** Konto firmowe (C2.12). Nie są to sekrety — klucz zostaje w Menedżerze poświadczeń. */
export interface EnterpriseSettings {
  /** OpenAI: identyfikator organizacji (`org-…`) — nagłówek `OpenAI-Organization`. */
  readonly organization?: string | null;
  /** OpenAI: identyfikator projektu (`proj_…`) — nagłówek `OpenAI-Project`. */
  readonly project?: string | null;
  /** Azure OpenAI: wersja API (`api-version`, np. `2024-10-21`). */
  readonly api_version?: string | null;
  /** Azure OpenAI: wdrożenia modeli w zasobie. */
  readonly deployments?: readonly AzureDeployment[];
}

/** Ostatni test konta w skrócie (te same pola co w `TestReport`, bez listy modeli). */
export interface AccountLastTest {
  readonly ok: boolean;
  /** `ok`, `invalid_key`, `restricted`, `no_credit`, `rate_limited`, `timeout`, `network`, `unsupported`. */
  readonly reason: string;
  readonly latency_ms: number | null;
  readonly status?: number | null;
  readonly provider_code?: string | null;
  readonly detail?: string | null;
}

/** Dane kreatora. `secret` trafia jednorazowo do backendu (Credential Manager); UI go nie przechowuje. */
export interface AddAccountInput {
  readonly provider_id: string;
  readonly label: string;
  readonly secret: string;
  readonly base_url: string | null;
  /** Konto firmowe (OpenAI: organizacja i projekt; Azure OpenAI: wersja API i wdrożenia). */
  readonly enterprise?: EnterpriseSettings;
}

/** Pozycja wspólnej kolejki wszystkich kont: model wybranego konta. */
export interface SharedQueueEntry {
  readonly account_id: string;
  readonly model: string;
}

/** Klasa zadań (`chat`, `code`, …) → wspólna kolejka w kolejności prób Routera. */
export type SharedQueues = Readonly<Record<string, readonly SharedQueueEntry[]>>;

// ── Dobór modelu według trudności ───────────────────────────────────────────────────────────────

export type RoutingLevel = 'easy' | 'medium' | 'hard';
export type RoutingRung = 'light' | 'standard' | 'strong';
export type RoutingStrategy = 'economy' | 'balanced' | 'quality';

/** Ustawienia doboru (plik `model-routing.json` rdzenia). */
export interface ModelRoutingSettings {
  readonly enabled: boolean;
  readonly strategy: RoutingStrategy;
  /** Poprawka po odrzuceniu odpowiedzi na mocniejszym modelu. */
  readonly escalate: boolean;
  /** Trudność sprawdzania odpowiedzi przez Krytyczkę. */
  readonly review: RoutingLevel;
  /** Przy równym szczeblu najpierw model lokalny. */
  readonly local_first: boolean;
  /** Szczeble ustawione ręcznie: `dostawca:model` → szczebel. */
  readonly rungs: Readonly<Record<string, RoutingRung>>;
  /** Dobór zmienia też kolejność kolejek ustawionych ręcznie. */
  readonly reorder_queues: boolean;
}

/** Model Routera z jego szczeblem. */
export interface RoutingModel {
  readonly candidate: string;
  readonly provider_name: string;
  readonly model: string;
  readonly local: boolean;
  readonly rung: RoutingRung;
  readonly automatic: RoutingRung;
}

export interface ModelRouting {
  readonly settings: ModelRoutingSettings;
  readonly models: readonly RoutingModel[];
}

/** Podgląd: jak Router potraktuje przykładowe zadanie. */
export interface RoutingPreview {
  readonly difficulty: RoutingLevel | null;
  readonly score: number | null;
  readonly reasons: string;
  readonly start: RoutingRung | null;
  readonly kept_queue: boolean;
  readonly order: readonly RoutingModel[];
}

export interface TestReport {
  readonly ok: boolean;
  readonly latency_ms: number | null;
  readonly models: readonly ModelInfo[];
  readonly error: string | null;
  /**
   * Rodzaj wyniku, gdy nie jest zwykłym sukcesem: `invalid_key`, `restricted` (klucz działa, ale
   * bez prawa do listy modeli), `no_credit`, `rate_limited`, `timeout`, `network`, `unsupported`.
   */
  readonly reason?: string | null;
  /** Kod HTTP odpowiedzi dostawcy. */
  readonly status?: number | null;
  /** Kod błędu dostawcy (np. `invalid_api_key`). */
  readonly provider_code?: string | null;
  /** Komunikat dostawcy w oryginale (bez sekretów). */
  readonly detail?: string | null;
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
  /** Zakresy pamięci (dokumenty `memory/<zakres>`; sesje prywatne poza eksportem). */
  readonly memory: readonly string[];
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
  readonly kind: 'session' | 'config' | 'persona' | 'cast' | 'memory';
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

/**
 * Podgląd paczki. `handle` — jednorazowy uchwyt pliku wydany przez rdzeń (natywny dialog albo
 * kopia z listy kopii zapasowych); UI nigdy nie podaje ścieżki. Następny krok (hasło, import)
 * zużywa uchwyt, a odpowiedź niesie nowy.
 */
export type InspectResult =
  | { readonly status: 'cancelled' }
  | { readonly status: 'needs_password'; readonly handle: string }
  | {
      readonly status: 'inspected';
      readonly handle: string;
      readonly manifest: PackageManifestSummary;
      readonly items: readonly DryRunItem[];
      readonly warnings: readonly string[];
      readonly migrations: readonly string[];
    };

export type ImportMode = 'add' | 'merge' | 'replace';
export type CollisionResolution = 'keep_local' | 'take_imported' | 'keep_both';

export interface ImportRequest {
  /** Uchwyt z ostatniego `InspectResult` (jednorazowy). */
  readonly handle: string;
  readonly mode: ImportMode;
  readonly resolutions: Readonly<Record<string, CollisionResolution>>;
  readonly password: string | null;
}

export interface ImportResult {
  /** Automatyczny snapshot przed importem → jednoklikowy rollback. */
  readonly snapshot_id: string;
  readonly imported: number;
  /** Pominięte w planie albo identyczne (bez nieudanych). */
  readonly skipped: number;
  /** Elementy, których nie udało się zapisać. */
  readonly failed: number;
}

// ── Uprawnienia (PLAN §8.3) ──────────────────────────────────────────────────────────────────────

export interface PermissionsState {
  readonly global: AutonomyLevel;
  readonly session: AutonomyLevel | null;
  readonly hello_enabled: boolean;
}

/**
 * Zmiana poziomu to intencja: podniesienie — Broker otwiera swoje okno i tam ją potwierdzasz
 * (`opened_broker`); obniżenie nie wymaga zgody i działa od razu (`applied`, `request_id` pusty).
 */
export interface BrokerIntentResult {
  readonly status: 'opened_broker' | 'applied';
  readonly request_id: string;
}

// ── Urządzenia (device-profile) ─────────────────────────────────────────────────────────────────

export type HwClass =
  'baseline' | 'standard_amd' | 'laptop_cuda' | 'strong' | 'integrated' | 'unknown';
export type VoiceProfileId = 'A' | 'B' | 'C' | 'D';

export interface GpuInfo {
  readonly vendor: string;
  readonly model: string;
  readonly vram_mb: number;
  readonly backends: readonly string[];
  /** Rola przy kilku kartach: modele główne, lekkie zadania albo nieużywana. */
  readonly role: 'heavy' | 'light' | 'spare';
  /** Karta zintegrowana — pamięć współdzielona z RAM. */
  readonly integrated: boolean;
  readonly shared_mb: number;
}

/** Stan jednej karty graficznej na żywo (pamięć w MiB). */
export interface GpuLiveCard {
  readonly index: number;
  readonly name: string;
  readonly total_mb: number;
  readonly used_mb: number;
  readonly free_mb: number;
  readonly load_pct: number | null;
  readonly temp_c: number | null;
  readonly power_w: number | null;
}

/** Program korzystający z karty; `alfa` — uruchomiony przez Alfę. */
export interface GpuLiveProcess {
  readonly pid: number;
  readonly name: string;
  readonly used_mb: number | null;
  readonly alfa: boolean;
}

/** Karty graficzne na żywo (`device_gpu_live`); `available: false` — pomiaru nie da się zrobić. */
export interface GpuLive {
  readonly available: boolean;
  readonly cards: readonly GpuLiveCard[];
  readonly processes: readonly GpuLiveProcess[];
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

// ── Modele lokalne (providers-local, PLAN §14.5) ────────────────────────────────────────────────

/** Model z manifestu `providers-local` (GGUF z SHA-256, bez kwantów IQ). */
export interface LocalModelInfo {
  readonly id: string;
  readonly name: string;
  readonly size_bytes: number;
  readonly installed: boolean;
  /** Model domyślny (onboarding pobiera go jednym kliknięciem). */
  readonly default: boolean;
  readonly downloading: boolean;
}

export type LocalDownloadState = 'downloading' | 'done' | 'failed' | 'cancelled';

/** Wynik importu jednej zmiennej środowiskowej (tylko nazwa zmiennej — nigdy wartość). */
export interface EnvImportResult {
  readonly provider: string;
  readonly provider_name: string;
  /** Nazwa zmiennej, np. `OPENAI_API_KEY`. */
  readonly var: string;
  readonly result: 'imported' | 'already_present' | 'not_set';
  /** Konto dodane albo już istniejące z tym kluczem. */
  readonly account_id: string | null;
}
