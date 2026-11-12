// PLIK GENEROWANY — nie edytuj ręcznie. Źródło: typy Rust w `crates/app-api` → schemat
// `packages/schemas/ipc.v1.json` → `node scripts/gen-ipc-types.mjs` (ADR 0013).

import type { AgentId } from '@alfa/ui-kit';

/** Autor tury: użytkownik albo jedna z agentek. */
export type Author = 'user' | AgentId;
/** Przeznaczenie modelu rozpoznane po nazwie (`ModelInfo.purpose`). */
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
/** Wielkość modelu (`ModelInfo.tier`). */
export type ModelTier = 'standard' | 'small' | 'tiny' | 'premium';
/** Rola karty przy kilku kartach: modele główne, lekkie zadania albo nieużywana (`GpuView.role`). */
export type GpuRole = 'heavy' | 'light' | 'spare';

/** Wersja kształtu schematu łącza. */
export const IPC_SCHEMA_VERSION = 1;

/** „O programie”. */
export interface AboutInfo {
  /** Data kompilacji wydania (`None` — build deweloperski). */
  readonly build_date: string | null;
  readonly channel: UpdateChannel;
  readonly commit: string | null;
  readonly licenses: readonly LicenseEntry[];
  /** Lista licencji z chwili generowania (`gen-licenses.mjs`). */
  readonly licenses_generated_at: string | null;
  readonly target: string;
  /** Czy aktualizacje są skonfigurowane (adres wydań + klucz minisign w wydaniu). */
  readonly updates_configured: boolean;
  readonly version: string;
}

/** Konto (bez sekretu). */
export interface Account {
  readonly assignments: AccountAssignment;
  /** Model, którego Alfa użyje do rozmowy bez kolejki (wybór automatyczny). */
  readonly auto_model?: string | null;
  /** Własny adres API konta (np. zasób Azure OpenAI — formularz firmowy pokazuje go bez edycji). */
  readonly base_url?: string | null;
  readonly cost_limit: AccountCostLimit;
  /**
   * Konto firmowe: organizacja i projekt OpenAI albo wersja API i wdrożenia Azure OpenAI
   * (nie sekrety; klucz zostaje w Menedżerze poświadczeń).
   */
  readonly enterprise?: EnterpriseSettings;
  readonly id: string;
  /**
   * Kiedy zapisano obecny klucz (dodanie konta albo ostatnia wymiana) — wiek klucza
   * i przypomnienie o wymianie liczy interfejs.
   */
  readonly key_saved_at?: string | null;
  readonly key_stored: boolean;
  readonly label: string;
  /** Wynik ostatniego testu — lista kont pokazuje powód, nie tylko „klucz odrzucony”. */
  readonly last_test?: AccountLastTest | null;
  readonly last_tested_at: string | null;
  readonly models: readonly ModelInfo[];
  readonly provider_id: string;
  /**
   * Kolejki modeli: klasa zadań (`chat`, `code`, …) → modele w kolejności prób. Brak klasy =
   * wybór automatyczny.
   */
  readonly queues?: Readonly<Record<string, readonly string[]>>;
  readonly state: AccountState;
}

/** Przypisania konta. */
export interface AccountAssignment {
  readonly agents: readonly AgentId[];
  readonly task_classes: readonly string[];
  readonly voice_stt: boolean;
  readonly voice_tts: boolean;
}

/** Limit kosztów konta. */
export interface AccountCostLimit {
  readonly enabled: boolean;
  readonly monthly: Money;
}

/** Ostatni test konta w skrócie (te same pola co w [`TestReport`], bez listy modeli). */
export interface AccountLastTest {
  readonly detail?: string | null;
  readonly latency_ms: number | null;
  readonly ok: boolean;
  readonly provider_code?: string | null;
  /** `ok`, `invalid_key`, `restricted`, `no_credit`, `rate_limited`, `timeout`, `network`, `unsupported`. */
  readonly reason: string;
  readonly status?: number | null;
}

/** Stan konta w UI. */
export type AccountState =
  'unconfigured' | 'testing' | 'ok' | 'invalid' | 'rate_limited' | 'disabled';

/** Kapsuła aktywności. */
export interface ActivityInfo {
  readonly agent: AgentId;
  readonly description: string;
  readonly session_id: string;
  readonly started_at: string;
  readonly step: number;
  readonly total_steps: number;
}

/** Dane kreatora konta. Sekret trafia wyłącznie do Credential Managera. */
export interface AddAccountInput {
  readonly base_url: string | null;
  /**
   * Konto firmowe (OpenAI: organizacja i projekt; Azure OpenAI: wersja API i wdrożenia).
   * Rdzeń sprawdza i oczyszcza je przy zapisie konta. Pole opcjonalne (zwykłe konto).
   */
  readonly enterprise?: EnterpriseSettings;
  readonly label: string;
  readonly provider_id: string;
  readonly secret: SecretInput;
}

/** Szkic agentki. */
export interface AgentDraft {
  /** Charakter. */
  readonly character: string | null;
  /** Token koloru z palety Kreatora. */
  readonly color: string | null;
  /** Odmiana imienia (domyślnie wyliczona dla imion żeńskich na „-a”). */
  readonly forms: NameForms | null;
  /** Glif (domyślnie pierwsza litera imienia). */
  readonly glyph: string | null;
  /** Identyfikator persony (domyślnie z imienia). */
  readonly id: string | null;
  /** Limity. */
  readonly limits: LimitsDraft;
  /** Imię. */
  readonly name: string | null;
  /** Rola. */
  readonly role: RoleDraft | null;
  /** Umiejętności (identyfikatory z biblioteki `skills`). */
  readonly skills: readonly string[];
  /** Głos v0. */
  readonly voice: VoiceDraft | null;
}

/** Przebieg agentki (nagłówek; kroki — `AgentRunDetail.steps` / zdarzenie `AgentStep`). */
export interface AgentRun {
  readonly agent: AgentId;
  /** Most CLI (`claude_code`, `codex`) — zdarzenia niezweryfikowane przez Alfę. */
  readonly bridge?: string | null;
  readonly budget: RunBudgetView;
  readonly finished_at: string | null;
  readonly goal: string;
  readonly id: string;
  /** Etykieta podprzebiegu („Krytyczka", „delegacja: …", „umiejętność: …"). */
  readonly label?: string | null;
  /** Przebieg-rodzic (podprzebieg: delegacja, Krytyczka, umiejętność). */
  readonly parent_id?: string | null;
  readonly session_id: string;
  readonly started_at: string;
  readonly state: RunState;
  readonly summary: string | null;
  /** Zadanie schedulera, w ramach którego trwa przebieg. */
  readonly task_id?: string | null;
  readonly turn_id: string | null;
  readonly usage: RunUsage;
  readonly workdir: string | null;
}

/** Przebieg z krokami (`agents_runs`). */
export interface AgentRunDetail {
  readonly run: AgentRun;
  readonly steps: readonly ReplayStep[];
}

/** Agentka w sesji. */
export interface AgentState {
  readonly activity: string | null;
  readonly id: AgentId;
  readonly role_ids: readonly string[];
  readonly status: AgentStatus;
}

/** Stan agentki. */
export type AgentStatus = 'idle' | 'speaking' | 'working' | 'waiting_approval';

/** Zdarzenie rdzeń → UI. */
export type AlfaEvent =
  | {
      readonly session_id: string;
      readonly turn: Turn;
      readonly type: 'TurnAppended';
    }
  | {
      readonly session_id: string;
      readonly status: TurnStatus;
      readonly turn_id: string;
      readonly type: 'TurnStatus';
    }
  | {
      readonly blocks: readonly RenderedBlock[];
      /**
       * Długość tekstu tury przed tą deltą (w jednostkach UTF-16, jak `String.length` w JS):
       * klient pomija część, którą już ma (np. z migawki `turns_list`), i wykrywa lukę.
       */
      readonly offset: number;
      readonly session_id: string;
      readonly text: string;
      readonly turn_id: string;
      readonly type: 'TextDelta';
    }
  | {
      readonly done: boolean;
      readonly elapsed_ms: number;
      readonly session_id: string;
      readonly turn_id: string;
      readonly type: 'ThinkingDelta';
    }
  | {
      readonly session_id: string;
      readonly step: ToolStep;
      readonly turn_id: string;
      readonly type: 'ToolCall';
    }
  | {
      readonly approval: ApprovalPending;
      readonly session_id: string;
      readonly turn_id: string;
      readonly type: 'ApprovalPending';
    }
  | {
      readonly session_id: string;
      readonly turn_id: string;
      readonly type: 'Usage';
      readonly usage: TurnUsage;
    }
  | {
      readonly reason: StopReason;
      readonly session_id: string;
      readonly turn_id: string;
      readonly type: 'Stop';
    }
  | {
      readonly error: TurnError;
      readonly session_id: string;
      readonly turn_id: string;
      readonly type: 'Error';
    }
  | {
      readonly session: SessionSummary;
      readonly type: 'SessionUpdated';
    }
  | {
      readonly session_id: string;
      readonly type: 'SessionRemoved';
    }
  | {
      readonly agents: readonly AgentState[];
      readonly session_id: string;
      readonly type: 'AgentsChanged';
    }
  | {
      readonly activity: ActivityInfo | null;
      readonly session_id: string;
      readonly type: 'ActivityChanged';
    }
  | {
      readonly costs: CostSummary;
      readonly session_id: string;
      readonly type: 'CostsChanged';
    }
  | {
      readonly status: SystemStatus;
      readonly type: 'SystemStatusChanged';
    }
  | {
      readonly event: TimelineEvent;
      readonly type: 'TimelineAppended';
    }
  | {
      readonly account: Account;
      readonly type: 'AccountChanged';
    }
  | {
      /** Poziom 0–1 po redukcji szumów (to, co słyszy rozpoznawanie mowy). */
      readonly level: number;
      /** Poziom surowy z mikrofonu (test mikrofonu) — widać, ile tła odcięto. */
      readonly raw?: number | null;
      readonly type: 'MicLevel';
    }
  | {
      /** Agentka, która mówi (puste przy ciszy po przerwaniu). */
      readonly agent: string;
      readonly level: number;
      readonly type: 'SpeechLevel';
    }
  | {
      /** Numer zdania (od 0). */
      readonly index: number;
      /** Liczba zdań w czytanym tekście. */
      readonly of: number;
      /** Tekst mówiony zdania (bez znaczników; kod zastąpiony opisem). */
      readonly text: string;
      /** Czytana tura. */
      readonly turn_id: string;
      readonly type: 'SpeechSegment';
    }
  | {
      readonly state: VoicePillState;
      readonly type: 'VoicePill';
    }
  | {
      readonly kind: ToastKind;
      readonly message: LocalizedText;
      readonly type: 'Toast';
    }
  | {
      readonly session_id: string;
      readonly type: 'OpenSession';
    }
  | {
      readonly run: AgentRun;
      readonly session_id: string;
      readonly type: 'AgentRunUpdated';
    }
  | {
      readonly run_id: string;
      readonly session_id: string;
      readonly step: ReplayStep;
      readonly type: 'AgentStep';
    }
  | {
      readonly status: VoiceStatus;
      readonly type: 'VoiceStatusChanged';
    }
  | {
      readonly bytes: number;
      readonly error: string | null;
      readonly model_id: string;
      readonly state: LocalDownloadState;
      readonly total: number | null;
      readonly type: 'LocalModelProgress';
    }
  | {
      readonly scope_key: string | null;
      readonly type: 'MemoryChanged';
    }
  | {
      readonly task: TaskInfo;
      readonly type: 'TaskUpdated';
    }
  | {
      readonly run: TriggerRunInfo;
      readonly type: 'TriggerFired';
    }
  | {
      readonly report: MarshalReport;
      readonly type: 'MarshalReportReady';
    }
  | {
      readonly status: GuiStatus;
      readonly type: 'GuiActivity';
    }
  | {
      readonly skill_id: string | null;
      readonly type: 'SkillsChanged';
    }
  | {
      readonly overall: HealthOverall;
      readonly pending: number;
      readonly type: 'HealthChanged';
    }
  | {
      readonly status: UpdatesView;
      readonly type: 'UpdateStatus';
    }
  | {
      readonly status: BrokerStatusView;
      readonly type: 'BrokerStatus';
    }
  | {
      readonly features: VoiceFeatures;
      readonly type: 'VoiceFeaturesChanged';
    }
  | {
      readonly done: number;
      readonly file: string;
      readonly item_id: string;
      readonly total: number | null;
      readonly type: 'ModelProgress';
    }
  | {
      readonly item: ModelItem;
      readonly type: 'ModelChanged';
    }
  | {
      readonly status: ReindexView;
      readonly type: 'ReindexStatus';
    }
  | {
      readonly state: CamerasState;
      readonly type: 'CamerasChanged';
    };

/** Dane startowe UI. */
export interface AppBootstrap {
  readonly active_session_id: string | null;
  readonly app_version: string;
  readonly layout: LayoutPrefs | null;
  readonly locale: Locale;
  readonly machine_name: string;
  readonly onboarding_done: boolean;
  readonly settings: Readonly<Record<string, SettingValue>>;
  readonly shortcut_overrides: Readonly<Record<string, string>>;
}

/** Błąd komendy: kod i komunikat PL gotowy do pokazania użytkownikowi. */
export interface AppError {
  /** Kod. */
  readonly code: ErrorCode;
  /**
   * Szczegóły do odczytu maszynowego: stałe klucze (`kind`, `id`, `field`, `reason`, `module`,
   * `provider`, `retry_after_s`), bez sekretów i treści użytkownika. Puste — pole pominięte
   * na łączu (kształt sprzed B.3).
   */
  readonly details?: ErrorDetails;
  /** Komunikat po polsku (bez sekretów). */
  readonly message: string;
}

/** Karta „czeka na zatwierdzenie" (UI tylko przenosi do okna Brokera). */
export interface ApprovalPending {
  /** Czy działa okno Brokera (Broker-UI); bez niego prośba czeka do limitu i kończy się odmową. */
  readonly broker_window: boolean;
  /** Kiedy prośba wygaśnie (limit czekania agentki). */
  readonly expires_at: string | null;
  readonly id: string;
  readonly reversible: boolean;
  readonly risk: RiskLevel;
  readonly status: ApprovalStatus;
  readonly what: string;
  readonly why: string;
}

/** Stan prośby o zatwierdzenie. */
export type ApprovalStatus = 'pending' | 'approved' | 'denied' | 'expired';

/** Akcja na pliku (intencja). */
export type ArtifactAction = 'open' | 'reveal' | 'copy' | 'save_as';

/** Artefakt (plik wyjściowy) sesji. */
export interface ArtifactInfo {
  readonly agent: AgentId | null;
  readonly created_at: string;
  readonly id: string;
  readonly mime: string;
  readonly name: string;
  readonly path: string;
  readonly session_id: string;
  readonly size_bytes: number;
  readonly versions: number;
}

/** Podgląd pliku: tekst jako zwykły tekst; obraz przez protokół zasobów. */
export type ArtifactPreview =
  | {
      readonly kind: 'text';
      readonly text: string;
      readonly truncated: boolean;
    }
  | {
      readonly kind: 'image';
      readonly src: string;
    }
  | {
      readonly kind: 'none';
    };

/** Co z treści załącznika trafi do modelu. */
export type AttachmentDelivery = 'full' | 'truncated' | 'metadata_only';

/**
 * Załącznik przygotowany w composerze (kopia w katalogu sesji `…\in`, jeszcze niewysłany).
 * Podgląd w UI przez protokół zasobów (`path`), nigdy bajty przez IPC.
 */
export interface AttachmentInfo {
  readonly bytes: number;
  readonly delivery: AttachmentDelivery;
  readonly id: string;
  readonly kind: AttachmentKind;
  readonly mime: string;
  readonly name: string;
  readonly path: string;
  readonly session_id: string;
  readonly tokens: number;
}

/** Rodzaj załącznika (podgląd w UI i sposób przekazania modelowi). */
export type AttachmentKind = 'image' | 'text' | 'document';

/** Powód odrzucenia pliku. */
export type AttachmentRejectReason =
  'too_large' | 'too_many' | 'total_too_large' | 'denied' | 'not_a_file' | 'unreadable' | 'empty';

/** Odrzucony plik (nazwa bez ścieżki). */
export interface AttachmentRejection {
  readonly name: string;
  readonly reason: AttachmentRejectReason;
}

/** Wynik dodania plików: przyjęte i odrzucone z powodem; `staged` = wszystkie przygotowane. */
export interface AttachmentsAdded {
  readonly added: readonly AttachmentInfo[];
  readonly rejected: readonly AttachmentRejection[];
  readonly staged: readonly AttachmentInfo[];
}

/** Urządzenie audio. */
export interface AudioDevice {
  readonly default: boolean;
  readonly id: string;
  readonly name: string;
}

/** Uwierzytelnienie. */
export type AuthKind = 'api_key' | 'oauth_cli' | 'none';

/** Poziom autonomii (PLAN §8.3). */
export type AutonomyLevel = 'L0' | 'L1' | 'L2' | 'L3' | 'L4';

/** Poziom autonomii (PLAN §8.3). Porządek rosnący: L0 < L1 < L2 < L3 < L4; domyślnie L3. */
export type AutonomyLevel2 = 'L0' | 'L1' | 'L2' | 'L3' | 'L4';

/**
 * Wdrożenie modelu w zasobie Azure OpenAI: Alfa wybiera model (`model`), a żądanie idzie na
 * adres wdrożenia (`name`).
 */
export interface AzureDeployment {
  /** Model, który wdrożenie udostępnia (np. `gpt-4o`); brak — model nazywa się jak wdrożenie. */
  readonly model?: string | null;
  /** Nazwa wdrożenia w zasobie (część adresu `/openai/deployments/{name}/…`). */
  readonly name: string;
}

/** Wynik próby przywrócenia (otwarcie, sumy kontrolne, dry-run — nic nie zapisuje). */
export interface BackupCheck {
  readonly app_version: string | null;
  readonly created_at: string | null;
  readonly encrypted: boolean;
  readonly file: string;
  readonly items: number;
  readonly message: string | null;
  readonly ok: boolean;
  readonly sessions: number;
}

/** Ustawienia kopii zapasowych (per maszyna, `%LOCALAPPDATA%\Alfa\state\backup.json`). */
export interface BackupConfig {
  readonly dir: string | null;
  readonly enabled: boolean;
  readonly include_artifacts: boolean;
  readonly include_logs: boolean;
  readonly interval_hours: number;
  readonly keep: number;
  readonly skip_on_battery: boolean;
}

/** Plik kopii w katalogu. */
export interface BackupEntry {
  readonly bytes: number;
  readonly created_at: string;
  readonly file: string;
  readonly path: string;
}

/** Stan kopii zapasowych. */
export interface BackupView {
  readonly config: BackupConfig;
  readonly entries: readonly BackupEntry[];
  readonly last_error: string | null;
  readonly last_run: string | null;
  readonly next_due: string | null;
  readonly password_set: boolean;
  readonly running: boolean;
}

/** Bateria. */
export interface BatteryDiag {
  readonly charge_pct: number | null;
  readonly cycles: number | null;
  /** Pojemność fabryczna i bieżąca pełna (mWh). */
  readonly design_mwh: number | null;
  readonly full_mwh: number | null;
  readonly on_ac: boolean | null;
}

/** Bateria. */
export interface BatteryView {
  readonly on_ac: boolean;
  readonly percent: number;
}

/** Rodzaj bloku treści. */
export type BlockKind = 'text' | 'code';

/** Przypięta odpowiedź (kopia treści z nazwą rozmowy, agentką i czasem przypięcia). */
export interface BoardItem {
  readonly agent: string;
  /** Treść jako HTML zsanitowany w Rust — tylko do `SanitizedHtml`. */
  readonly html_sanitized: string;
  readonly id: string;
  readonly pinned_at: string;
  readonly session_id: string;
  readonly session_title: string;
  /** Treść (Markdown) — do kopiowania. */
  readonly text: string;
  readonly turn_id: string;
}

/** Stan bezpiecznika silnika głównego. */
export type BreakerStateView = 'closed' | 'open' | 'half_open';

/** Wynik sprawdzenia konta mostu — tylko stan, sposób logowania i plan (bez e-maila, tokenów). */
export interface BridgeAccount {
  /** Kiedy sprawdzono. */
  readonly checked_at: string;
  /** Zdanie dla użytkownika: co działa albo co zrobić. */
  readonly detail: string;
  /** Sposób logowania (np. „konto claude.ai”). */
  readonly method: string | null;
  /** Plan podany przez narzędzie (np. „Max”). */
  readonly plan: string | null;
  readonly state: BridgeAccountState;
}

/** Stan konta mostu CLI. */
export type BridgeAccountState = 'logged_in' | 'logged_out' | 'unknown' | 'not_installed' | 'error';

/** Karta zgodności trasy mostu. */
export interface BridgeCard {
  /** Ostatnie sprawdzenie konta (`bridges_check`); `None` — jeszcze nie sprawdzano. */
  readonly account: BridgeAccount | null;
  readonly allowed: readonly string[];
  /** Most obsługiwany przez Alfę (`claude_code`, `codex`); `null` — trasa tylko w rejestrze. */
  readonly bridge: string | null;
  /** Trasa zabroniona albo nieobsługiwana — włącznik nieaktywny. */
  readonly can_enable: boolean;
  readonly detected: boolean;
  readonly enabled: boolean;
  readonly forbidden: readonly string[];
  /** Polecenie logowania do wpisania przez użytkownika w terminalu. */
  readonly login_command: string | null;
  /** `cli-p`, `sdk`, `api`. */
  readonly mode: string;
  readonly name: string;
  readonly path: string | null;
  /** Wersje przypięte w konfiguracji (`[agent_backends.bridges.<most>] pinned_versions`). */
  readonly pinned: readonly string[];
  readonly program: string | null;
  readonly provider: string;
  /** Przypięcie z rejestru zgodności (`cli_pinned_version`). */
  readonly registry_pin: string | null;
  readonly route_id: string;
  /** Jawna zgoda na uruchamianie z harmonogramu: limit na dobę (0 = brak zgody). */
  readonly schedule_per_day: number;
  readonly sources: readonly BridgeSource[];
  /** Wpis rejestru nieświeży — trasa zdegradowana do szarej. */
  readonly stale: boolean;
  readonly status: ComplianceStatus;
  /** Data weryfikacji (`YYYY-MM-DD`). */
  readonly verified_at: string | null;
  /** Wersja wykryta (`--version`). */
  readonly version: string | null;
  /** Wykryta wersja jest przypięta (inaczej most odmówi startu). */
  readonly version_ok: boolean;
}

/**
 * Wynik „Zaloguj w terminalu": terminal otwarty w katalogu domowym, polecenie do skopiowania
 * (Alfa go nie wykonuje).
 */
export interface BridgeLogin {
  readonly command: string;
  readonly cwd: string;
  readonly opened: boolean;
}

/** Źródło (regulamin, dokumentacja) z rejestru zgodności. */
export interface BridgeSource {
  readonly quote: string;
  readonly url: string;
}

/** Wynik intencji Brokera. */
export interface BrokerIntentResult {
  readonly request_id: string;
  readonly status: BrokerIntentStatus;
}

/**
 * Stan intencji Brokera: `opened_broker` — prośba czeka w oknie Brokera; `applied` — zmiana
 * niewymagająca zatwierdzenia (obniżenie poziomu) została zastosowana od razu.
 */
export type BrokerIntentStatus = 'opened_broker' | 'applied';

/** Stan łącza z Brokerem. */
export type BrokerLinkState = 'connected' | 'connecting' | 'lost';

/** Gdzie działa Broker (ADR 0003). */
export type BrokerMode = 'service' | 'portable' | 'in_process' | 'unavailable';

/** Stan Brokera dla Ustawień → Uprawnienia i bezpieczeństwo oraz banera w oknie głównym. */
export interface BrokerStatusView {
  /** Działa okno zatwierdzeń (Broker-UI) — karta „czeka na zatwierdzenie” może do niego przenieść. */
  readonly approval_window: boolean;
  /** Wyjaśnienie trybu albo przyczyny zerwania (zwykły tekst). */
  readonly detail: LocalizedText | null;
  /** Broker na osobnym koncie (usługa) — tylko wtedy pełna izolacja z ADR 0003. */
  readonly isolated: boolean;
  readonly mode: BrokerMode;
  readonly state: BrokerLinkState;
  /** `Ctrl+Shift+F12` obsługuje `alfa-watchdog` poza UI (inaczej skrót rejestruje aplikacja). */
  readonly watchdog: boolean;
}

/** Agentka z biblioteki Kreatora. */
export interface BuilderAgentInfo {
  readonly autonomy: AutonomyLevel;
  readonly color: string;
  readonly hash: string;
  readonly name: string;
  readonly persona: string;
  readonly role: string;
  /**
   * Identyfikator roli do przypisania w rozmowie; `role` jest nazwą do wyświetlenia.
   * Brak w starszych odpowiedziach nie uprawnia klienta do zgadywania ID.
   */
  readonly role_id?: string | null;
}

/** Raport testu na sucho (zaliczony = warunek zapisu tego samego hasha). */
export interface BuilderDryRun {
  readonly hash: string;
  readonly passed: boolean;
  readonly steps: readonly BuilderDryStep[];
}

/** Krok testu na sucho. */
export interface BuilderDryStep {
  readonly expected: DryOutcome;
  readonly outcome: DryOutcome;
  readonly tool: string;
  readonly why: string;
}

/** Polityka Kreatora (formularz). */
export interface BuilderPolicyView {
  readonly ceiling: AutonomyLevel;
  readonly groups: readonly string[];
  readonly max_steps: number;
  readonly model_policies: readonly string[];
  readonly palette: readonly string[];
  readonly voices: readonly string[];
}

/** Podgląd persony i roli zbudowanego manifestu. */
export interface BuilderPreview {
  readonly autonomy: AutonomyLevel;
  readonly character: string;
  readonly color: string;
  readonly forms: readonly string[];
  readonly fs_write: readonly string[];
  readonly glyph: string;
  readonly groups: readonly string[];
  readonly hash: string;
  readonly max_steps: number;
  readonly memory_scope: string;
  readonly name: string;
  readonly persona_id: string;
  readonly read_only: boolean;
  readonly retain_days: number;
  readonly role_id: string;
  readonly role_name: string;
  readonly system_prompt: string;
  readonly tools: readonly string[];
  readonly voice: string;
  readonly warnings: readonly string[];
}

/** Szkic z rozmowy + pytania o braki. */
export interface BuilderProposal {
  readonly draft: AgentDraft;
  readonly questions: readonly string[];
}

/** Zapisana agentka. */
export interface BuilderSaved {
  readonly hash: string;
  readonly persona: string;
  readonly role: string;
}

/** Polecenie wbudowane (opis i przykłady). */
export interface BuiltinCommand {
  readonly examples: readonly string[];
  readonly title: string;
}

/** Dopasowanie z uzasadnieniem (`reason` — dla `tight` i `too_weak`). */
export interface BundleFit {
  readonly kind: BundleFitKind;
  readonly reason: LocalizedText | null;
}

/** Dopasowanie pakietu do sprzętu tej maszyny. */
export type BundleFitKind = 'fits' | 'too_weak' | 'tight';

/** Pozycja pakietu (wariant silnika dobrany dla tej maszyny) z jej stanem. */
export interface BundleItemView {
  /** Do pobrania w aplikacji (inaczej — instalacja ręczna; pakiet jej nie pobiera). */
  readonly downloadable: boolean;
  /** Silnik zapasowy (CPU) — używany, gdy wersja na kartę graficzną nie wystartuje. */
  readonly fallback: boolean;
  readonly id: string;
  readonly kind: ModelItemKind;
  readonly name: string;
  readonly size_bytes: number;
  readonly state: ModelItemState;
}

/**
 * Wymagania pakietu. Progi w MB z tolerancją raportowania systemu (np. 16 GB RAM ≈ ≥ 15 000 MB,
 * karta 8 GB ≈ ≥ 7 500 MB); `text` — opis do pokazania z wartościami nominalnymi.
 */
export interface BundleRequirements {
  /** Karta konieczna; inaczej `min_vram_mb` i `min_cpu_cores` to alternatywy („karta albo procesor”). */
  readonly gpu_required: boolean;
  /** Rdzenie fizyczne procesora; `null` — bez wymagania. */
  readonly min_cpu_cores: number | null;
  readonly min_ram_mb: number;
  /** Pamięć karty graficznej (CUDA albo Vulkan); `null` — karta niepotrzebna. */
  readonly min_vram_mb: number | null;
  readonly text: LocalizedText;
}

/** Stan pakietu na tej maszynie (wyliczany ze stanów jego pozycji). */
export type BundleState =
  'not_installed' | 'partial' | 'installed' | 'corrupt' | 'downloading' | 'needs_trust';

/** Klatka z kamery jako PNG (base64), już pomniejszona. */
export interface CameraFrame {
  readonly camera: string;
  readonly height: number;
  readonly png_base64: string;
  /** Chwila zrobienia klatki (ms od 1970). */
  readonly taken_ms: number;
  readonly width: number;
}

/** Rodzaj kamery. */
export type CameraKindView = 'usb' | 'network';

/** Stan strumienia kamery. */
export type CameraStateView = 'asleep' | 'opening' | 'live' | 'failed' | 'unplugged' | 'closing';

/** Kamera na liście. */
export interface CameraView {
  /** Opis błędu przy stanie `failed`. */
  readonly detail: string | null;
  readonly enabled: boolean;
  readonly height: number;
  readonly id: string;
  readonly kind: CameraKindView;
  readonly name: string;
  readonly state: CameraStateView;
  /** Rozmiar obrazu kamery; 0, dopóki kamera nie działa. */
  readonly width: number;
}

/** Stan kamer: przełącznik główny i lista. */
export interface CamerasState {
  readonly cameras: readonly CameraView[];
  readonly enabled: boolean;
}

/** Rodzaj artefaktu w kanwie (C2.8): Markdown jako zwykły DOM, HTML i SVG w izolowanej ramce. */
export type CanvasKind = 'markdown' | 'html' | 'svg';

/** Szablon obsady. */
export type CastTemplateId = 'standard' | 'solo' | 'coding' | 'research';

/** Pozycja historii zmian. */
export interface ChangeItem {
  /** Tytuł sekcji wpisu (np. „Interfejs”, „Głos”). */
  readonly section: string;
  readonly severity: ChangeSeverity;
  /** Treść pozycji jako zwykły tekst. */
  readonly text: string;
}

/** Wpis historii zmian — jedna wersja programu. */
export interface ChangeLogEntry {
  /** Data wydania `RRRR-MM-DD`, jeśli wpis ją podaje. */
  readonly date: string | null;
  readonly items: readonly ChangeItem[];
  /** Zdanie wstępu wpisu. */
  readonly summary: string;
  readonly version: string;
}

/** Waga pozycji w historii zmian (kolor w interfejsie: czerwony, pomarańczowy, zielony). */
export type ChangeSeverity = 'critical' | 'serious' | 'normal';

/** Rodzaj testu. */
export type CheckKind =
  | {
      readonly test: 'cpu_single';
    }
  | {
      readonly test: 'cpu_multi';
    }
  | {
      readonly test: 'memory';
    }
  | {
      readonly target: string;
      readonly test: 'disk';
    }
  | {
      readonly test: 'network';
    }
  | {
      readonly test: 'integrity';
    };

/** Wynik jednego testu. */
export interface CheckResult {
  readonly duration_ms: number;
  readonly kind: CheckKind;
  readonly metrics: readonly Metric[];
  /** Uwagi i powody oceny (po polsku). */
  readonly notes: readonly string[];
  readonly verdict: Verdict;
}

/** Przebieg testów (zapisywany w historii). */
export interface CheckRun {
  /** RFC 3339. */
  readonly at: string;
  /** Najgorszy wynik (bez pominiętych; same pominięte — `Skipped`). */
  readonly overall: Verdict;
  readonly results: readonly CheckResult[];
}

/** Zmiana profilu Dziecko; PIN wpisuje rodzic, sprawdza go wyłącznie Sędzia Cieni. */
export type ChildLockRequest =
  | {
      readonly new: SecretInput;
      readonly old: SecretInput | null;
      readonly op: 'set_pin';
    }
  | {
      readonly on: boolean;
      readonly op: 'enable';
      readonly pin: SecretInput;
    }
  | {
      readonly minutes: number | null;
      readonly op: 'open';
      readonly pin: SecretInput;
      readonly session_id: string | null;
    }
  | {
      readonly op: 'close';
    }
  | {
      readonly op: 'verify';
      readonly pin: SecretInput;
    }
  | {
      readonly on: boolean;
      readonly op: 'transcripts';
      readonly pin: SecretInput;
    };

/** Profil Dziecko (plan 50 §4.4): stan kłódki trybów pracy (bez PIN-u). */
export interface ChildLockState {
  readonly attempts_left: number;
  /** Blokada po błędnych próbach do (ms od 1970). */
  readonly blocked_until_ms?: number | null;
  /** Kłódka otwarta „do końca rozmowy” (tylko jedna rozmowa). */
  readonly conversation: boolean;
  /** Ta rozmowa (gdy rdzeń ją zna) — interfejs pokazuje kłódkę w każdej innej. */
  readonly conversation_id?: string | null;
  readonly enabled: boolean;
  /** Kłódka otwarta teraz PIN-em rodzica. */
  readonly open: boolean;
  /** Do kiedy otwarta (ms od 1970); `None` — zamknięta albo do zamknięcia Alfy. */
  readonly open_until_ms?: number | null;
  readonly pin_set: boolean;
  /** Zapis kłódki poza zasięgiem konta dziecka; `false` — ochrona umowna. */
  readonly tamper_resistant: boolean;
  /** Rodzic włączył pełną treść rozmów w dzienniku (dziecko widzi stały znak). */
  readonly transcripts: boolean;
  /** Stan nieznany (zerwane łącze z Sędzią); interfejs pokazuje ostatni znany stan i „nieznany”. */
  readonly unknown: boolean;
}

/** Rozwiązanie kolizji. */
export type CollisionResolution = 'keep_local' | 'take_imported' | 'keep_both';

/** Czy licencja pozwala na użycie komercyjne. */
export type CommercialUse = 'yes' | 'no' | 'conditional' | 'unknown';

/** Zgodność protokołu. */
export type CompatKind = 'openai' | 'anthropic' | 'native';

/** Status zgodności. */
export type ComplianceStatus = 'green' | 'gray' | 'forbidden' | 'unverified';

/** Raport porządkowania pamięci (Strażniczka). */
export interface ConsolidationReport {
  readonly budget_denied: boolean;
  readonly conflicts: number;
  readonly created: number;
  readonly errors: readonly string[];
  readonly expired: number;
  readonly interrupted: string | null;
  readonly llm_calls: number;
  readonly manual: boolean;
  readonly merged: number;
  readonly proposals: number;
  readonly resolved: number;
  readonly run: string;
  readonly scopes: number;
  /** Dlaczego nie wystartował (bateria, tryb gry, okno, bezczynność, wyłączona…). */
  readonly skipped: string | null;
  readonly started_at: string;
}

/** Zużycie okna kontekstu. */
export interface ContextUsage {
  readonly compacted: boolean;
  readonly max_tokens: number;
  readonly used_tokens: number;
}

/** Format eksportu rozmowy. */
export type ConversationFormat = 'markdown' | 'html' | 'json';

/** Limit miesięczny w podsumowaniu kosztów. */
export interface CostLimitView {
  readonly enabled: boolean;
  readonly monthly: Money;
}

/** Podsumowanie kosztów. */
export interface CostSummary {
  readonly context: ContextUsage;
  readonly day: Money;
  readonly fx: FxView;
  readonly limit: CostLimitView;
  readonly month: Money;
  readonly session: Money;
}

/** Procesor. */
export interface CpuDiag {
  /** Taktowanie bazowe i bieżące (MHz). */
  readonly base_mhz: number | null;
  readonly cores: number;
  readonly current_mhz: number | null;
  /** Pamięć podręczna L2 / L3 (KB). */
  readonly l2_kb: number | null;
  readonly l3_kb: number | null;
  /** Obciążenie w chwili pomiaru (%). */
  readonly load_pct: number | null;
  readonly name: string;
  readonly threads: number;
  /** Wirtualizacja włączona w firmware. */
  readonly virtualization: boolean | null;
}

/** Procesor. */
export interface CpuView {
  readonly cores: number;
  readonly model: string;
  readonly threads: number;
}

/** Podgląd wyrażenia cron (następne uruchomienia w strefie Europe/Warsaw). */
export interface CronPreview {
  readonly error: string | null;
  readonly next: readonly string[];
  readonly valid: boolean;
}

/** Waluta kwoty. */
export type Currency = 'PLN' | 'USD';

/** Własne polecenie: frazy → stała odpowiedź. */
export interface CustomCommand {
  readonly enabled: boolean;
  /** Identyfikator (pusty przy dodawaniu — nadaje rdzeń). */
  readonly id: string;
  readonly mode: QuickMatchMode;
  readonly name: string;
  readonly phrases: readonly string[];
  /** Wyższy wygrywa (−10…10). */
  readonly priority: number;
  /** Odpowiedź (Markdown) z `{reszta}` i nazwanymi grupami wyrażenia. */
  readonly reply: string;
}

/** Trasa do dostawcy (API albo most CLI). */
export interface DataRoute {
  readonly enabled: boolean;
  readonly id: string;
  /** Kody jurysdykcji (np. `US`, `EU`, `CN`); pusta lista — nieznana. */
  readonly jurisdiction: readonly string[];
  /** `api`, `cli-p` albo `sdk`. */
  readonly mode: string;
  /** Tagi prywatności (np. `eu`, `cn-may-train`, `unknown`). */
  readonly privacy: readonly string[];
  /** Czy trasa obsłuży sesję prywatną. */
  readonly private_allowed: boolean;
  /** Powód decyzji dla sesji prywatnej (po polsku). */
  readonly private_reason: string;
  readonly provider: string;
  /** Wpis rejestru nieświeży. */
  readonly stale: boolean;
  /** Czy trasa obsłuży zwykłą sesję. */
  readonly standard_allowed: boolean;
  /** Status po degradacji: `green`, `grey`, `forbidden`. */
  readonly status: string;
  /** Ryzyko trenowania na danych: `no_train`, `may_train`, `unknown`. */
  readonly training: string;
  /** Wpis katalogu niezweryfikowany. */
  readonly unverified: boolean;
}

/** Wszystkie trasy. */
export interface DataRoutesView {
  readonly routes: readonly DataRoute[];
}

/** Wartość szczegółu: tekst (identyfikator, wartość wyliczenia), liczba albo flaga. */
export type DetailValue = boolean | number | string;

/** Profil urządzenia. */
export interface DeviceProfile {
  readonly machine: MachineView;
  readonly measured_at: string;
  readonly recommendation: RecommendationView;
}

/** Postęp trwających testów. */
export interface DiagProgress {
  /** Klucz bieżącego testu (`disk:C:`). */
  readonly current: string;
  /** Wyniki zakończonych testów tego przebiegu. */
  readonly done: readonly CheckResult[];
  /** Numer bieżącego testu (od 0) i liczba testów. */
  readonly index: number;
  readonly total: number;
}

/** Stan diagnostyki. */
export interface DiagView {
  /** Klucze dostępnych testów (`cpu_single`, `disk:C:`…). */
  readonly available: readonly string[];
  /** Ocena spójności ostatniego zapisu (kondycja, miejsce, zdarzenia, temperatury). */
  readonly integrity: CheckResult | null;
  /** Ostatni przebieg testów. */
  readonly last_run: CheckRun | null;
  /** Ostatni zapis parametrów. */
  readonly latest: Snapshot | null;
  /** Zapis w toku. */
  readonly recording: boolean;
  /** Trwające testy. */
  readonly running: DiagProgress | null;
  readonly runs: number;
  /** Liczba zapisów i przebiegów w historii. */
  readonly snapshots: number;
}

/** Akcja dyktowania. */
export type DictationAction =
  | {
      readonly kind: 'start';
    }
  | {
      readonly kind: 'stop';
    }
  | {
      readonly kind: 'toggle';
    }
  | {
      readonly kind: 'undo';
    }
  | {
      readonly kind: 'save_profile';
      readonly profile: DictationProfile;
    }
  | {
      readonly app: string;
      readonly kind: 'remove_profile';
    };

/** Profil dyktowania dla aplikacji (nazwa pliku procesu, np. `notepad.exe`). */
export interface DictationProfile {
  readonly app: string;
  readonly block_enter: boolean;
  readonly capitalize_start: boolean;
}

/** Stan dyktowania. */
export type DictationStateView = 'unavailable' | 'idle' | 'active' | 'paused';

/** Dyktowanie do dowolnej aplikacji. */
export interface DictationView {
  readonly app: string | null;
  readonly can_undo: boolean;
  readonly pending_chars: number;
  readonly preview: string | null;
  readonly profiles: readonly DictationProfile[];
  readonly reason: LocalizedText | null;
  readonly shortcut: string;
  readonly state: DictationStateView;
  readonly typed_chars: number;
}

/** Rodzaj linii diffu. */
export type DiffKind = 'same' | 'added' | 'removed';

/** Linia diffu. */
export interface DiffLine {
  readonly kind: DiffKind;
  readonly text: string;
}

/** Dysk fizyczny. */
export interface DiskDiag {
  /** Magistrala (NVMe, SATA, USB…). */
  readonly bus: string | null;
  /** Błędy tego dysku w dzienniku zdarzeń (okno [`crate::EVENT_WINDOW_DAYS`]). */
  readonly event_errors: number;
  readonly health: Health;
  readonly media: MediaKind;
  readonly model: string;
  /** Numer dysku w systemie (`Harddisk<N>` w dzienniku zdarzeń). */
  readonly number: number | null;
  /** Stan operacyjny podany przez system (np. „OK”, „Degraded”). */
  readonly operational: string | null;
  readonly power_on_hours: number | null;
  readonly read_errors: number | null;
  /** Dysk wymienny (np. USB). */
  readonly removable: boolean;
  readonly serial_tail: string | null;
  readonly size_mb: number;
  readonly temperature_c: number | null;
  /** Litery woluminów na tym dysku. */
  readonly volumes: readonly string[];
  /** Zużycie (% — SSD), temperatura (°C), godziny pracy, błędy odczytu/zapisu (licznik SMART). */
  readonly wear_pct: number | null;
  readonly write_errors: number | null;
}

/** Dysk. */
export interface DiskInfo {
  readonly free_bytes: number;
  readonly low: boolean;
}

/** Oczekiwany / przewidywany wynik kroku testu na sucho. */
export type DryOutcome = 'allowed' | 'ask' | 'denied';

/** Element podglądu importu. */
export interface DryRunItem {
  readonly diff: ItemDiff;
  readonly key: string;
  readonly kind: DryRunKind;
  readonly label: string;
}

/** Rodzaj elementu paczki. */
export type DryRunKind = 'session' | 'config' | 'persona' | 'cast' | 'memory';

/** Embedder wyszukiwania (`[search.embedder] model`). */
export interface EmbedderView {
  /** Model w użyciu (`lexical`, gdy wybrany model nie jest zainstalowany albo się nie załadował). */
  readonly active: string;
  /** Wybór w ustawieniach (`lexical` albo identyfikator modelu). */
  readonly configured: string;
  readonly dims: number;
  readonly error: string | null;
  /** Identyfikator w indeksie (`model_id/wymiar` — zmiana = przebudowa wektorów). */
  readonly index_id: string;
}

/** Wynik próby jednego silnika. */
export interface EngineCheckView {
  /** Wyjaśnienie: przyczyna błędu albo uwaga do udanej próby. */
  readonly detail: string | null;
  /** Czas próby (ms). */
  readonly ms: number;
  /** Silnik odpowiedział. */
  readonly ok: boolean;
}

/** Akcja na silnikach rozmowy. */
export type EnginesAction =
  | {
      readonly kind: 'refresh';
    }
  | {
      readonly kind: 'reset_breaker';
    }
  | {
      readonly kind: 'self_test';
    };

/** Ostatnia nagrana fraza rejestracji (bez audio). */
export interface EnrollSampleView {
  readonly accepted: boolean;
  readonly duration_ms: number;
  readonly level_db: number;
  readonly message: LocalizedText | null;
  readonly quality: SampleQuality;
}

/** Ustawienia konta firmowego. Puste = zwykłe konto (pole pomijane w zapisie). */
export interface EnterpriseSettings {
  /** Azure OpenAI: wersja API (`api-version`, np. `2024-10-21`). */
  readonly api_version?: string | null;
  /** Azure OpenAI: wdrożenia modeli w zasobie. */
  readonly deployments?: readonly AzureDeployment[];
  /** OpenAI: identyfikator organizacji (`org-…`) — nagłówek `OpenAI-Organization`. */
  readonly organization?: string | null;
  /** OpenAI: identyfikator projektu (`proj_…`) — nagłówek `OpenAI-Project`. */
  readonly project?: string | null;
}

/** Co stało się ze zmienną. */
export type EnvImportKind = 'imported' | 'already_present' | 'not_set';

/** Wynik importu jednej zmiennej środowiskowej (tylko nazwa zmiennej — nigdy wartość). */
export interface EnvImportResult {
  /** Konto dodane albo już istniejące z tym kluczem. */
  readonly account_id: string | null;
  readonly provider: string;
  readonly provider_name: string;
  readonly result: EnvImportKind;
  /** Nazwa zmiennej, np. `OPENAI_API_KEY`. */
  readonly var: string;
}

/** Kod błędu komendy (stabilny, do testów i ewentualnej logiki UI). */
export type ErrorCode =
  | 'not_found'
  | 'invalid_input'
  | 'unavailable'
  | 'forbidden'
  | 'storage'
  | 'secrets'
  | 'provider'
  | 'internal';

/** Słownik szczegółów błędu (pusty = pole `details` nie jest wysyłane). */
export type ErrorDetails = Readonly<Record<string, DetailValue>>;

/** Zestaw ewaluacyjny (bez przypadków). */
export interface EvalSuiteView {
  readonly id: string;
  readonly integrity_ok: boolean;
  readonly problems: readonly string[];
  readonly status: string;
  readonly thresholds: number;
  readonly version: string;
  readonly wave: string;
}

/** Werdykt bramki (wynik zbiorczy — bez danych przypadków holdoutu). */
export interface EvalVerdictView {
  readonly at: string;
  readonly passed: boolean;
  readonly stage: string;
  readonly suite: string;
  readonly summary: string;
}

/** Wyniki evali. */
export interface EvalsView {
  readonly available: boolean;
  readonly holdout_suites: number;
  readonly reason: string | null;
  readonly suites: readonly EvalSuiteView[];
  readonly verdicts: readonly EvalVerdictView[];
}

/** Liczba zdarzeń rodzaju. */
export interface EventCount {
  readonly count: number;
  readonly kind: EventKind;
}

/** Ramka zdarzeń `alfa://events`: paczka z numerami kolejnymi (plan v6 B.2, ADR 0019). */
export interface EventFrame {
  /** Znacznik uruchomienia rdzenia (numery z innego uruchomienia nic nie znaczą). */
  readonly epoch: string;
  /** Zdarzenia w kolejności. */
  readonly events: readonly AlfaEvent[];
  /**
   * `seq` poprzedniej ramki (0 — to pierwsza); inna wartość niż ostatnia znana klientowi
   * oznacza zgubioną ramkę.
   */
  readonly prev: number;
  /** Numer ostatniego zdarzenia w tej ramce. */
  readonly seq: number;
  /** Wersja ramki. */
  readonly v: number;
}

/** Rodzaj zdarzenia awarii. */
export type EventKind =
  'unexpected_shutdown' | 'bugcheck' | 'hardware' | 'disk' | 'usb' | 'app_crash' | 'other';

/** Poziom zdarzenia (rosnąco). */
export type EventLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'audit';

/** Zdarzenie z dziennika (skrót; treść systemowa obcięta). */
export interface EventLine {
  /** RFC 3339. */
  readonly at: string;
  readonly id: number;
  /** Rodzaj (klucz `EventKind`). */
  readonly kind: EventKind;
  readonly message: string;
  readonly source: string;
}

/** Podsumowanie zdarzeń z ostatnich dni. */
export interface EventSummary {
  /** Liczba zdarzeń w każdym rodzaju. */
  readonly counts: readonly EventCount[];
  /** Najnowsze zdarzenia (≤ 30). */
  readonly recent: readonly EventLine[];
  readonly window_days: number;
}

/** Wynik `events_resume`: brakujące ramki albo informacja, że trzeba wczytać stan od nowa. */
export interface EventsResume {
  /**
   * `false` — brakujących ramek nie da się już odtworzyć (inne uruchomienie albo wypadły
   * z bufora); klient wczytuje stan od nowa i liczy od `latest_seq`.
   */
  readonly complete: boolean;
  /** Znacznik bieżącego uruchomienia rdzenia. */
  readonly epoch: string;
  /** Ramki po `after_seq`, w kolejności. */
  readonly frames: readonly EventFrame[];
  /** `seq` ostatniej wydanej ramki. */
  readonly latest_seq: number;
}

/** Żądanie eksportu. Hasło nigdy nie trafia do logów (`Debug` redaguje). */
export interface ExportRequest {
  readonly password: SecretInput | null;
  readonly scope: ExportScope;
}

/** Wynik eksportu. */
export type ExportResult =
  | {
      readonly status: 'cancelled';
    }
  | {
      readonly bytes: number;
      readonly files: number;
      readonly path: string;
      readonly status: 'saved';
    };

/** Zakres eksportu. */
export interface ExportScope {
  readonly artifacts: boolean;
  readonly casts: boolean;
  readonly config_common: boolean;
  readonly config_machine: boolean;
  readonly logs: boolean;
  /** Zakresy pamięci (nazwy dokumentów, np. `global.ndjson`, `project/dom.ndjson`). */
  readonly memory: readonly string[];
  readonly personas: boolean;
  readonly sessions: readonly string[];
}

/** Kurs walut użyty do przeliczeń. */
export interface FxView {
  readonly date: string;
  readonly stale: boolean;
  readonly usd_pln: number;
}

/** Karta graficzna. */
export interface GpuDiag {
  /** Data sterownika (`RRRR-MM-DD`). */
  readonly driver_date: string | null;
  readonly driver_version: string | null;
  readonly name: string;
  /** Stan urządzenia (np. „OK”, „Error”). */
  readonly status: string | null;
  readonly vram_mb: number | null;
}

/** Odczyt kart na żywo. */
export interface GpuLive {
  /** Pomiar dostępny (dziś: karty NVIDIA ze sterownikiem); `false` — listy są puste. */
  readonly available: boolean;
  readonly cards: readonly GpuLiveCard[];
  readonly processes: readonly GpuLiveProcess[];
}

/** Stan jednej karty. */
export interface GpuLiveCard {
  /** Pamięć wolna (MiB). */
  readonly free_mb: number;
  readonly index: number;
  /** Obciążenie (%); `None` — karta go nie podaje. */
  readonly load_pct: number | null;
  readonly name: string;
  /** Pobór mocy (W). */
  readonly power_w: number | null;
  /** Temperatura (°C). */
  readonly temp_c: number | null;
  /** Pamięć razem (MiB). */
  readonly total_mb: number;
  /** Pamięć zajęta (MiB). */
  readonly used_mb: number;
}

/** Program korzystający z karty. */
export interface GpuLiveProcess {
  /** Program uruchomiony przez Alfę (silnik modelu, mowy albo sam program). */
  readonly alfa: boolean;
  /** Nazwa pliku programu (bez katalogu). */
  readonly name: string;
  readonly pid: number;
  /** Zajęta pamięć karty (MiB); `None` — system jej nie podaje. */
  readonly used_mb: number | null;
}

/** Karta graficzna. */
export interface GpuView {
  readonly backends: readonly string[];
  /** Karta zintegrowana (pamięć współdzielona z RAM). */
  readonly integrated: boolean;
  readonly model: string;
  /**
   * Rola na maszynie z kilkoma kartami: `heavy` (modele główne), `light` (lekkie zadania
   * na karcie zintegrowanej), `spare` (nieużywana przez Alfę).
   */
  readonly role: GpuRole;
  /** Pamięć współdzielona z systemem (MB). */
  readonly shared_mb: number;
  readonly vendor: string;
  readonly vram_mb: number;
}

/**
 * Zgoda „zawsze w tym zakresie” w panelu uprawnień (komenda `permissions_grants`); odwołanie —
 * `permissions_revoke_grant` (zawsze bez okna Sędziego: tylko zawęża uprawnienia).
 */
export interface GrantView {
  /** Agentka (brak — cała sesja). */
  readonly agent: string | null;
  readonly granted_at_ms: number;
  /** Numer zgody w Sędzim Cieni. */
  readonly id: string;
  /** Zakres po ludzku („połączenie z internetem: strona x.example.org”). */
  readonly scope: string;
  readonly session_id: string;
  /** Do kiedy; `None` — do odwołania. */
  readonly until_ms: number | null;
}

/** Akcja GUI agentki (bez treści wpisywanej — tylko rodzaj, cel i wynik). */
export interface GuiAction {
  readonly agent: string;
  readonly at: string;
  readonly duration_ms: number | null;
  readonly id: number;
  readonly session_id: string;
  readonly status: GuiActionStatus;
  readonly summary: string;
  readonly target: string | null;
  readonly title: string;
  readonly tool: string;
}

/** Stan akcji GUI agentki. */
export type GuiActionStatus = 'running' | 'ok' | 'denied' | 'failed' | 'cancelled';

/** Kto steruje teraz (wskaźnik w pasku tytułu). */
export interface GuiControl {
  readonly agent: string;
  readonly session_id: string;
  readonly since: string;
  readonly tool: string;
}

/** Ostatni zrzut agentki (zamaskowany w porcie: okna Alfy/Brokera, deny-lista, pola haseł). */
export interface GuiScreenshot {
  readonly data_url: string;
  readonly info: GuiShotInfo;
}

/** Metadane ostatniego zrzutu (piksele — tylko `gui_screenshot`, nigdy w zdarzeniach). */
export interface GuiShotInfo {
  readonly agent: string;
  readonly at: string;
  readonly black_frame: boolean;
  readonly height: number;
  readonly masked: number;
  readonly session_id: string;
  readonly width: number;
}

/** Stan panelu „Ekran". */
export interface GuiStatus {
  readonly actions: readonly GuiAction[];
  readonly available: boolean;
  readonly control: GuiControl | null;
  readonly reason: LocalizedText | null;
  readonly screenshot: GuiShotInfo | null;
  readonly taken_over: boolean;
}

/** Kondycja według systemu (Storage Spaces / SMART). */
export type Health = 'healthy' | 'warning' | 'unhealthy' | 'unknown';

/** Co wymaga człowieka. */
export interface HealthHumanAction {
  readonly id: number;
  readonly mitigated: boolean;
  readonly title: string;
  readonly what: string;
}

/** Incydent Diagnosty. */
export interface HealthIncident {
  readonly count: number;
  readonly id: number;
  readonly kind: string;
  readonly last_at: string;
  readonly status: string;
  readonly target: string;
  readonly title: string;
}

/** Wiersz modułu. */
export interface HealthModule {
  readonly detail: string | null;
  readonly health: ModuleHealth;
  readonly lifecycle: string;
  readonly module: string;
  readonly version: string;
}

/** Stan ogólny. */
export type HealthOverall = 'ok' | 'degraded' | 'failing' | 'safe_mode';

/** Propozycja naprawy czekająca na zgodę (Jądro → wyłącznie Broker-UI). */
export interface HealthProposal {
  readonly diff: readonly string[];
  readonly id: number;
  readonly kernel: boolean;
  readonly rationale: string;
  readonly risk: RiskView;
  readonly rollback_plan: readonly string[];
  readonly title: string;
}

/** Wykonana naprawa (z „Cofnij"). */
export interface HealthRepair {
  readonly at: string;
  readonly diff: readonly string[];
  readonly id: number;
  readonly title: string;
  readonly undoable: boolean;
}

/** Raport „Zdrowie systemu". */
export interface HealthView {
  readonly generated_at: string;
  readonly incidents: readonly HealthIncident[];
  readonly modules: readonly HealthModule[];
  readonly needs_human: readonly HealthHumanAction[];
  readonly overall: HealthOverall;
  readonly pending: readonly HealthProposal[];
  readonly problems: readonly string[];
  readonly repaired: readonly HealthRepair[];
  readonly safe_mode: string | null;
}

/** Trafienie wyszukiwania: dział i fragment tekstu. */
export interface HelpHit {
  readonly id: string;
  /** Nagłówek sekcji z trafieniem (pusty — przed pierwszym nagłówkiem). */
  readonly section: string;
  readonly snippet: string;
  readonly title: string;
}

/** Dział z treścią: HTML zsanitowany w Rust (pulldown-cmark + ammonia, ADR 0009). */
export interface HelpPage {
  readonly html_sanitized: string;
  readonly id: string;
  readonly title: string;
}

/** Dział instrukcji. */
export interface HelpTopic {
  readonly id: string;
  readonly title: string;
}

/** Klasa sprzętu. */
export type HwClass =
  'baseline' | 'standard_amd' | 'laptop_cuda' | 'strong' | 'unknown' | 'integrated';

/** Tryb importu. */
export type ImportMode = 'add' | 'merge' | 'replace';

/** Żądanie importu. */
export interface ImportRequest {
  /** Jednorazowy uchwyt z ostatniego `InspectResult`. */
  readonly handle: string;
  readonly mode: ImportMode;
  readonly password: SecretInput | null;
  readonly resolutions: Readonly<Record<string, CollisionResolution>>;
}

/** Wynik importu. */
export interface ImportResult {
  /** Elementy, których nie udało się zapisać. */
  readonly failed: number;
  readonly imported: number;
  /** Pominięte w planie albo identyczne (bez nieudanych). */
  readonly skipped: number;
  readonly snapshot_id: string;
}

/** Zablokowana próba (bez treści wartości). */
export interface ImproverBlocked {
  readonly at: string;
  readonly source: string;
  readonly target: string;
  readonly violation: string;
}

/** Zmiana konfiguracji w propozycji Ulepszacza. */
export interface ImproverChange {
  readonly key: string;
  readonly new: unknown;
  readonly old: unknown;
  readonly ring: string;
  readonly safety: string;
}

/** Szkic zgłoszenia R3 (kod — tylko do przeglądu człowieka). */
export interface ImproverIssue {
  readonly at: string;
  readonly body: string;
  readonly path: string;
  readonly title: string;
}

/** Propozycja Ulepszacza. */
export interface ImproverProposalView {
  readonly can_approve: boolean;
  readonly can_rollback: boolean;
  readonly changes: readonly ImproverChange[];
  readonly created_at: string;
  readonly digest: string;
  readonly id: number;
  readonly needs_signature: boolean;
  readonly note: string | null;
  readonly rationale: string;
  readonly ring: string;
  readonly safety: string;
  readonly source: string;
  readonly stage: string;
  readonly title: string;
}

/** Panel Ulepszacza. */
export interface ImproverView {
  readonly blocked: readonly ImproverBlocked[];
  readonly idle_cycle: boolean;
  readonly issues: readonly ImproverIssue[];
  readonly last_cycle: string | null;
  readonly proposals: readonly ImproverProposalView[];
}

/**
 * Wynik podglądu paczki. `handle` — jednorazowy uchwyt pliku dla następnego kroku (hasło,
 * import); UI nigdy nie podaje ścieżki paczki.
 */
export type InspectResult =
  | {
      readonly status: 'cancelled';
    }
  | {
      readonly handle: string;
      readonly status: 'needs_password';
    }
  | {
      readonly handle: string;
      readonly items: readonly DryRunItem[];
      readonly manifest: PackageManifestSummary;
      readonly migrations: readonly string[];
      readonly status: 'inspected';
      readonly warnings: readonly string[];
    };

/** Rodzaj intencji kroku (akcja, którą właściciel wykonuje sam). */
export type IntentKind = 'open_in_terminal' | 'confirm_delete_permanent';

/** Różnica elementu paczki. */
export type ItemDiff = 'new' | 'same' | 'changed' | 'collision';

/** Układ okna (per maszyna). */
export interface LayoutPrefs {
  /** Tryb skupienia (F11) — przywracany po ponownym uruchomieniu; brak w starszym zapisie = wyłączony. */
  readonly focus: boolean;
  readonly left_collapsed: boolean;
  readonly left_width: number;
  readonly right_width: number;
  readonly sessions: Readonly<Record<string, SessionPanels>>;
}

/** Karta licencji pozycji katalogu. */
export interface LicenseCard {
  /** Licencja wymaga wskazania autora albo dołączenia jej tekstu przy rozpowszechnianiu. */
  readonly attribution: boolean;
  /** Autor albo właściciel praw i źródło. */
  readonly author: LocalizedText;
  readonly commercial: CommercialUse;
  /** Licencja ma zapis sprawdzenia w dokumentacji projektu („Zapis licencji:” w uwagach); inaczej — „do potwierdzenia przez człowieka”. */
  readonly confirmed: boolean;
  /** Identyfikator licencji (SPDX, gdy istnieje; inaczej nazwa umowna, np. `OpenRAIL-M`). */
  readonly id: string;
  /** Pełna nazwa licencji (albo kilku — silnik i dołączone biblioteki). */
  readonly name: string;
  /** Warunki, ograniczenia użycia i sprawy otwarte (także znak wodny mowy syntetycznej). */
  readonly notes: LocalizedText;
  /** Źródło licencji: adres jej tekstu albo strony, na której autor ją podaje. */
  readonly url: string;
  /** Głosy w tej pozycji (pusto — pozycja nie mówi albo nie ma osobnych głosów). */
  readonly voices: readonly VoiceLicense[];
}

/** Licencja zależności. */
export interface LicenseEntry {
  /** Wyrażenie SPDX (np. `MIT OR Apache-2.0`). */
  readonly license: string;
  readonly name: string;
  readonly source: LicenseSource;
  readonly version: string;
}

/** Źródło licencji. */
export type LicenseSource = 'cargo' | 'npm';

/** Limity agentki. */
export interface LimitsDraft {
  /** Poziom autonomii (≤ sufit polityki; `None` = sufit). */
  readonly autonomy: AutonomyLevel2 | null;
  /** Budżet przebiegu (≤ sufit polityki; `None` = domyślny). */
  readonly budget: RunBudget | null;
  /** Zakresy zapisu (`%USERPROFILE%\Pobrane\**`), wyłącznie w profilu użytkownika. */
  readonly fs_write: readonly string[];
  /** Zakres pamięci (`agent` albo `session`). */
  readonly memory_scope: string | null;
  /** Retencja pamięci (dni, 1–365). */
  readonly retain_days: number | null;
  /** Wyzwalacze (cron, 5 pól) — dane; aktywacja osobno w module wyzwalaczy. */
  readonly triggers: readonly string[];
}

/** Stan pobierania modelu lokalnego (zdarzenie `LocalModelProgress`). */
export type LocalDownloadState = 'downloading' | 'done' | 'failed' | 'cancelled';

/** Model lokalny z manifestu `providers-local` (onboarding, Ustawienia → Modele lokalne). */
export interface LocalModelInfo {
  readonly default: boolean;
  readonly downloading: boolean;
  readonly id: string;
  readonly installed: boolean;
  readonly name: string;
  readonly size_bytes: number;
}

/** Język interfejsu. */
export type Locale = 'pl' | 'en';

/** Tekst w dwóch językach interfejsu. */
export interface LocalizedText {
  readonly en: string;
  readonly pl: string;
}

/** System operacyjny i komputer. */
export interface MachineDiag {
  /** Wersja BIOS/UEFI. */
  readonly bios: string | null;
  /** Producent i model komputera. */
  readonly model: string | null;
  /** Nazwa systemu (np. „Windows 11 Pro”). */
  readonly os: string;
  /** Wersja i kompilacja. */
  readonly os_version: string;
  /** Czas od uruchomienia (s). */
  readonly uptime_s: number;
}

/** Maszyna. */
export interface MachineView {
  readonly battery: BatteryView | null;
  readonly cpu: CpuView;
  readonly gpus: readonly GpuView[];
  readonly id: string;
  readonly name: string;
  readonly npu: string | null;
  readonly os: string;
  readonly ram_mb: number;
}

/** Propozycja reguł. */
export interface MarshalProposalInfo {
  readonly conflicts: readonly string[];
  readonly created_at: string;
  readonly id: number;
  readonly rejected: readonly MarshalRejected[];
  readonly rules: readonly MarshalRuleInfo[];
  /** `pending`, `approved`, `rejected`. */
  readonly status: string;
  readonly text: string;
}

/** Odrzucony szkic (nigdy nie wejdzie w życie). */
export interface MarshalRejected {
  readonly draft: unknown;
  readonly errors: readonly string[];
}

/** Raport dzienny Marszałka (relacjonuje Dyrygentka). */
export interface MarshalReport {
  readonly day: string;
  readonly escalations: number;
  readonly failed: number;
  readonly submitted: number;
  readonly succeeded: number;
  readonly text: string;
}

/** Reguła Marszałka (opis zawężeń po polsku + reguła źródłowa). */
export interface MarshalRuleInfo {
  readonly description: string;
  readonly effects: readonly string[];
  readonly id: string;
  readonly rule: unknown;
  readonly when: readonly string[];
}

/** Stan Marszałka (Ustawienia → Agentki → Reguły). */
export interface MarshalState {
  /** Polityka efektywna (sufit ∩ reguły) w punktach. */
  readonly effective: readonly string[];
  readonly proposals: readonly MarshalProposalInfo[];
  readonly rules: readonly MarshalRuleInfo[];
  /** Tłumacz poleceń (model) dostępny — bez niego tylko reguły z edytora. */
  readonly translator: boolean;
}

/** Zasięg „Max dostęp” (plan 50 §8.1): bez pytań na modelach lokalnych, w chmurze albo wszystkich. */
export type MaxAccess = 'local' | 'cloud' | 'all';

/** Stan połączenia z serwerem. */
export type McpConnState = 'disabled' | 'idle' | 'connected' | 'failed';

/** Zmienna środowiskowa serwera (wartość jawna albo tajna — wtedy tylko nazwa). */
export interface McpEnvVar {
  readonly name: string;
  readonly secret: boolean;
  /** Wartość jawna; `None` dla tajnej (w Menedżerze poświadczeń Windows). */
  readonly value: string | null;
}

/** Dane formularza serwera (dodanie albo zmiana). */
export interface McpServerInput {
  readonly args: readonly string[];
  readonly command: string;
  readonly cwd: string | null;
  readonly enabled: boolean;
  /** Zmienne; tajna z `value: None` zachowuje zapisaną wartość. */
  readonly env: readonly McpEnvVar[];
  /** Pusty — nowy serwer (identyfikator z nazwy). */
  readonly id: string | null;
  readonly name: string;
  readonly timeout_ms: number | null;
  readonly trust: McpTrust;
}

/** Wynik sprawdzenia serwera: stan po próbie i lista narzędzi. */
export interface McpServerTools {
  readonly server: McpServerView;
  readonly tools: readonly McpToolView[];
}

/** Serwer MCP na liście. */
export interface McpServerView {
  readonly args: readonly string[];
  /** Polecenie tak, jak wpisał je użytkownik (`npx`, `uvx`, pełna ścieżka). */
  readonly command: string;
  readonly cwd: string | null;
  readonly enabled: boolean;
  readonly env: readonly McpEnvVar[];
  /** Czytelny powód ostatniego niepowodzenia. */
  readonly error: string | null;
  readonly id: string;
  readonly name: string;
  /** Pełna ścieżka znalezionego programu; `None` — nie znaleziono w PATH. */
  readonly resolved: string | null;
  /** Nazwa i wersja z `initialize` (niezaufane). */
  readonly server_info: string | null;
  readonly state: McpConnState;
  readonly timeout_ms: number;
  readonly tools_approved: number;
  /** Liczba narzędzi z ostatniej listy i ile z nich ma zgodę. */
  readonly tools_total: number;
  readonly trust: McpTrust;
}

/** Stan zgody na narzędzie. */
export type McpToolState = 'approved' | 'auto_approved' | 'new' | 'changed';

/** Narzędzie serwera MCP. */
export interface McpToolView {
  /** Opis od serwera (niezaufany, skrócony). */
  readonly description: string | null;
  /** Odcisk bieżącej definicji (podawany przy zgodzie). */
  readonly fingerprint: string;
  readonly name: string;
  /** Nazwy argumentów ze schematu (podgląd). */
  readonly params: readonly string[];
  readonly state: McpToolState;
  /** Opis zawiera sygnały wstrzyknięcia poleceń. */
  readonly suspicious: boolean;
  readonly title: string | null;
}

/** Poziom zaufania serwera (ustawia wyłącznie użytkownik). */
export type McpTrust = 'untrusted' | 'reviewed' | 'trusted';

/** Rodzaj nośnika. */
export type MediaKind = 'ssd' | 'hdd' | 'unknown';

/** Pamięć operacyjna. */
export interface MemDiag {
  readonly available_mb: number;
  readonly modules: readonly MemModule[];
  /** Plik stronicowania: rozmiar i użycie (MB). */
  readonly pagefile_mb: number | null;
  readonly pagefile_used_mb: number | null;
  readonly total_mb: number;
}

/** Moduł pamięci. */
export interface MemModule {
  readonly capacity_mb: number;
  readonly manufacturer: string | null;
  readonly part: string | null;
  readonly slot: string | null;
  readonly speed_mhz: number | null;
}

/** Pozycja podglądu kaskady. */
export interface MemoryCascadeItem {
  readonly id: string;
  /** `target`, `version` albo `derived`. */
  readonly reason: string;
  readonly scope_key: string;
  /** Początek treści (do 120 znaków). */
  readonly text: string;
}

/** Edycja = nowa wersja (pola `null` bez zmian; pusty `subject` usuwa temat). */
export interface MemoryEdit {
  readonly confidence: number | null;
  readonly subject: string | null;
  readonly text: string | null;
}

/** „Dlaczego to pamiętam". */
export interface MemoryExplanation {
  readonly derived: readonly string[];
  readonly item: MemoryItem;
  readonly journal: readonly MemoryJournalEntry[];
  readonly merged: readonly string[];
  readonly reasons: readonly string[];
  readonly sources: readonly MemorySourceLink[];
  /** Historia wersji od najstarszej (łącznie z tym wpisem). */
  readonly versions: readonly MemoryItem[];
}

/** Podgląd kaskady przed zapomnieniem. */
export interface MemoryForgetPreview {
  readonly remove: readonly MemoryCascadeItem[];
  /** Wpisy, które wrócą do stanu aktywnego (były zastąpione przez usuwane). */
  readonly revive: readonly string[];
  /** Zakres zostanie usunięty w całości (crypto-shredding bazy). */
  readonly shred: boolean;
  readonly target: MemoryForgetTarget;
}

/** Raport zapomnienia (weryfikowalny). */
export interface MemoryForgetReport {
  readonly derived: number;
  readonly fts_rows: number;
  readonly journal_records: number;
  readonly removed: number;
  readonly revived: number;
  readonly shredded: readonly string[];
  readonly stale_exports: readonly string[];
  readonly vectors: number;
  readonly versions: number;
}

/** Co zapomnieć (kaskadowo). */
export type MemoryForgetTarget =
  | {
      readonly id: string;
      readonly target: 'entry';
    }
  | {
      readonly scope: string;
      readonly target: 'scope';
    };

/** Wpis pamięci. */
export interface MemoryItem {
  readonly confidence: number;
  readonly created_at: string;
  /** Jak powstał (`extracted`, `summary`, `skill`, `promoted`, `edited`, `imported`). */
  readonly derivation: string | null;
  readonly entities: readonly string[];
  readonly expires_at: string | null;
  readonly id: string;
  readonly layer: MemoryLayer;
  readonly pinned: boolean;
  readonly scope: MemoryScopeRef;
  readonly scope_key: string;
  readonly score: number | null;
  readonly session_id: string | null;
  readonly source: MemorySourceKind;
  /** Agentka (pochodzenie `agent`) albo źródło (treść niezaufana, import). */
  readonly source_detail: string | null;
  readonly state: MemoryState;
  readonly subject: string | null;
  readonly text: string;
  readonly trusted: boolean;
  readonly turn: number | null;
  readonly version: number;
}

/** Rekord dziennika zmian. */
export interface MemoryJournalEntry {
  readonly at: string;
  readonly entries: readonly string[];
  readonly id: string;
  /** `create`, `supersede`, `merge`, `expire`, `mark_consolidated`, `conflict`, `edit`, `promote`. */
  readonly kind: string;
  readonly note: string;
  /** Przebieg porządkowania (`null` = zmiana użytkownika). */
  readonly run: string | null;
  readonly scope_key: string;
  /** Czy da się cofnąć (wygaszenia są nieodwracalne). */
  readonly undoable: boolean;
  readonly undone: boolean;
}

/** Warstwa pamięci. */
export type MemoryLayer = 'working' | 'episodic' | 'semantic' | 'procedural';

/** Strona wyników. */
export interface MemoryPage {
  readonly items: readonly MemoryItem[];
  readonly total: number;
}

/** Zapytanie Inspektora (filtry łączone AND; puste listy = bez filtra). */
export interface MemoryQuery {
  readonly layers: readonly MemoryLayer[];
  readonly limit: number;
  readonly offset: number;
  readonly pinned: boolean | null;
  readonly scopes: readonly string[];
  readonly states: readonly MemoryState[];
  readonly text: string | null;
  readonly trusted: boolean | null;
}

/** Zakres z danymi (lista Inspektora). */
export interface MemoryScopeInfo {
  readonly active: number;
  /** Nazwa dokumentu w paczce `.alfa` (`null` — sesja prywatna, poza eksportem). */
  readonly document: string | null;
  readonly entries: number;
  readonly key: string;
  readonly label: string;
  readonly pending: number;
  readonly scope: MemoryScopeRef;
}

/** Rodzaj zakresu pamięci. */
export type MemoryScopeKind = 'session' | 'project' | 'agent' | 'global';

/** Zakres pamięci (`id` = sesja / projekt / agentka; `null` dla globalnej). */
export interface MemoryScopeRef {
  readonly id: string | null;
  readonly kind: MemoryScopeKind;
}

/** Pochodzenie wpisu. */
export type MemorySourceKind = 'user' | 'agent' | 'untrusted' | 'import';

/** Źródło wpisu w „dlaczego to pamiętam". */
export interface MemorySourceLink {
  readonly exists: boolean;
  readonly id: string;
  readonly state: MemoryState | null;
}

/** Stan wpisu. */
export type MemoryState = 'active' | 'pending' | 'superseded' | 'expired';

/** Stan pamięci dla Inspektora (porządkowanie, licznik bezczynności, oczekujące). */
export interface MemoryStatus {
  readonly consolidation_enabled: boolean;
  /** Licznik bezczynności dostępny (bez niego harmonogram nocny nie startuje; ręcznie — tak). */
  readonly idle_available: boolean;
  readonly last: ConsolidationReport | null;
  /** Model lokalny dla porządkowania (bez niego — tylko reguły deterministyczne). */
  readonly model_available: boolean;
  readonly pending: number;
  readonly window: string;
}

/** Wynik cofnięcia zmiany z dziennika. */
export interface MemoryUndoResult {
  readonly removed: number;
  readonly restored: number;
  readonly skipped: number;
}

/** Zmierzona wartość. */
export interface Metric {
  /** Większa wartość jest lepsza. */
  readonly higher_is_better: boolean;
  /** Klucz (`seq_write_mbps`, `latency_ms`…). */
  readonly key: string;
  /** Etykieta po polsku. */
  readonly label: string;
  readonly unit: string;
  readonly value: number;
}

/** Stan mikrofonu (uprawnienie/obecność). */
export type MicAvailability = 'ok' | 'denied' | 'missing';

/** Stan mikrofonu w pigułce. */
export type MicState =
  'off' | 'listening' | 'hearing' | 'processing' | 'speaking' | 'muted' | 'dnd';

/** Pakiet „Modele lokalne i silniki” w skali ocen 1–6 (6 — wzorcowy, 1 — minimalny) dla tej maszyny. */
export interface ModelBundle {
  readonly fit: BundleFit;
  readonly id: string;
  readonly installed: number;
  readonly items: readonly BundleItemView[];
  /** Co najwyżej tyle zostało do pobrania (pozycje brakujące, wstrzymane, uszkodzone). */
  readonly missing_bytes: number;
  readonly name: LocalizedText;
  readonly quality: readonly QualityNote[];
  /** Ocena 1–6. */
  readonly rating: number;
  /** Najwyższy pakiet, który pasuje do tej maszyny bez kompromisów. */
  readonly recommended: boolean;
  readonly requirements: BundleRequirements;
  /** Łączny rozmiar pobrań pozycji. */
  readonly size_bytes: number;
  readonly state: BundleState;
  readonly summary: LocalizedText;
  readonly total: number;
}

/**
 * Karta modelu (AI Act art. 50): skąd pochodzi, do czego służy, czego nie umie i czy tworzy
 * treść oznaczaną jako wygenerowaną przez AI.
 */
export interface ModelCard {
  /** Tworzy treść (tekst albo mowę) oznaczaną jako wygenerowaną przez AI. */
  readonly generative: boolean;
  /** Ograniczenia i ryzyka. */
  readonly limits: LocalizedText;
  /** Autor i kraj pochodzenia, podstawa modelu. */
  readonly origin: LocalizedText;
  /** Przeznaczenie w Alfie. */
  readonly purpose: LocalizedText;
}

/** Plik pozycji (pobierany albo wynikowy). */
export interface ModelFileView {
  readonly name: string;
  /** SHA-256 przypięty w katalogu (`null` — do przypięcia przez człowieka). */
  readonly pinned_sha256: string | null;
  /** SHA-256 policzony przy pobraniu (karta TOFU) albo zapisany przy instalacji. */
  readonly sha256: string | null;
  readonly size_bytes: number;
  readonly url: string;
}

/** Model konta. */
export interface ModelInfo {
  readonly context_tokens: number | null;
  readonly id: string;
  readonly kinds: readonly ModelKind[];
  readonly name: string;
  /**
   * Przeznaczenie rozpoznane po nazwie: `chat`, `reasoning`, `embedding`, `speech`,
   * `transcription`, `realtime`, `image`, `video`, `moderation`, `other`.
   */
  readonly purpose: ModelPurpose;
  /** Przypięta migawka z datą w nazwie (`…-2024-08-06`). */
  readonly snapshot: boolean;
  /** Wielkość: `standard`, `small`, `tiny`, `premium`. */
  readonly tier: ModelTier;
}

/** Pozycja katalogu z jej stanem na tej maszynie. */
export interface ModelItem {
  /** Embedder aktywny w wyszukiwaniu (tylko `embed`). */
  readonly active: boolean;
  /** Karta modelu (pochodzenie, przeznaczenie, ograniczenia). */
  readonly card: ModelCard | null;
  /** Adresy, rozmiary i licencja potwierdzone przez człowieka (inaczej „do potwierdzenia”). */
  readonly confirmed: boolean;
  /** Pozycja do pobrania w aplikacji (inaczej — instalacja ręczna wg opisu). */
  readonly downloadable: boolean;
  readonly error: string | null;
  readonly files: readonly ModelFileView[];
  readonly id: string;
  readonly kind: ModelItemKind;
  /** Licencja (SPDX albo opis warunków). */
  readonly license: string;
  /** Karta licencji silnika, modelu i jego głosów (C5.7). */
  readonly license_card: LicenseCard | null;
  readonly name: string;
  readonly note: LocalizedText;
  /** Wszystkie pliki mają przypięty SHA-256 (bez karty TOFU). */
  readonly pinned: boolean;
  readonly progress: ModelProgressView | null;
  readonly size_bytes: number;
  /** Strona źródła (repozytorium, karta modelu). */
  readonly source: string;
  readonly state: ModelItemState;
  /** Katalog docelowy na tej maszynie. */
  readonly target: string;
}

/** Rodzaj pozycji katalogu. */
export type ModelItemKind =
  'llm' | 'stt' | 'tts' | 'vad' | 'wake' | 'speaker' | 'embed' | 'sidecar';

/** Stan pozycji: brak → (kolejka) → pobieranie → [zgoda TOFU] → instalacja → zainstalowana. */
export type ModelItemState =
  | 'missing'
  | 'queued'
  | 'downloading'
  | 'installing'
  | 'failed'
  | 'paused'
  | 'needs_trust'
  | 'installed'
  | 'external'
  | 'corrupt';

/** Rodzaj modelu. */
export type ModelKind = 'chat' | 'stt' | 'tts' | 'embeddings' | 'vision';

/** Profil modelu sesji. */
export type ModelProfile = 'local' | 'hybrid' | 'cloud';

/** Postęp pobierania pliku. */
export interface ModelProgressView {
  readonly done: number;
  readonly file: string;
  readonly total: number | null;
}

/** Ustawienia i modele, które Router bierze pod uwagę. */
export interface ModelRouting {
  readonly models: readonly RoutingModel[];
  readonly settings: ModelRoutingSettings;
}

/** Ustawienia doboru. */
export interface ModelRoutingSettings {
  /** Dobór włączony. */
  readonly enabled: boolean;
  /** Poprawka po odrzuceniu odpowiedzi na mocniejszym modelu. */
  readonly escalate: boolean;
  /** Przy równym szczeblu najpierw model lokalny. */
  readonly local_first: boolean;
  /**
   * Dobór zmienia też kolejność kolejek ustawionych ręcznie (domyślnie nie — kolejka ręczna
   * ma pierwszeństwo).
   */
  readonly reorder_queues: boolean;
  /** Trudność sprawdzania odpowiedzi (Krytyczka). */
  readonly review: RoutingLevel;
  /** Szczeble ustawione ręcznie: `dostawca:model` → szczebel. */
  readonly rungs: Readonly<Record<string, RoutingRung>>;
  readonly strategy: RoutingStrategy;
}

/** Lista pozycji, embedder i przebudowa (`models_list`). */
export interface ModelsView {
  readonly embedder: EmbedderView;
  readonly items: readonly ModelItem[];
  /** Limit równoległych pobrań. */
  readonly parallel: number;
  readonly reindex: ReindexView;
}

/** Stan modułu w rejestrze. */
export type ModuleHealth = 'healthy' | 'degraded' | 'unhealthy' | 'not_started' | 'disabled';

/** Moduł na liście. */
export interface ModuleRow {
  /** Moduł opcjonalny, który można wyłączyć (od następnego uruchomienia). */
  readonly can_disable: boolean;
  readonly capabilities: readonly string[];
  readonly cpu_pct: number;
  /** Powód stanu (ograniczenie, błąd). */
  readonly detail: string | null;
  /** Wyłączony w ustawieniach. */
  readonly disabled: boolean;
  readonly id: string;
  /** `inproc`, `process`, `wasm`. */
  readonly isolation: string;
  /** `service`, `tool`, `provider`, `voice-engine`, `agent-pack`, `ui-panel`. */
  readonly kind: string;
  /** `always`, `lazy`, `on-demand`. */
  readonly lifecycle: string;
  /** Moduł Jądra (Broker, watchdog, audyt, aktualizacje, `core-*`) — bez wyłączania. */
  readonly protected: boolean;
  readonly provides: readonly string[];
  readonly ram_mb: number;
  readonly requires: readonly string[];
  /** Pominięty przy tym uruchomieniu (wyłączony, gdy program startował). */
  readonly skipped: boolean;
  /** `disabled`, `unloaded`, `loading`, `ready`, `degraded`, `failed`. */
  readonly state: string;
  readonly version: string;
  readonly vram_mb: number | null;
}

/** Lista modułów. */
export interface ModulesView {
  readonly modules: readonly ModuleRow[];
  /** Suma budżetów RAM modułów niewyłączonych (MB). */
  readonly ram_total_mb: number;
  /** Ustawienia różnią się od stanu z uruchomienia — zmiana zadziała po ponownym uruchomieniu. */
  readonly restart_needed: boolean;
}

/** Kwota w jednostkach drobnych (grosze / centy). */
export interface Money {
  readonly currency: Currency;
  readonly minor: number;
}

/** Odmiana imienia przez przypadki (do rozpoznawania adresatki i poleceń: „Delto”, „Delcie”…). */
export interface NameForms {
  /** Biernik („Deltę”). */
  readonly accusative: string;
  /** Celownik („Delcie”). */
  readonly dative: string;
  /** Dopełniacz („Delty”). */
  readonly genitive: string;
  /** Narzędnik („Deltą”). */
  readonly instrumental: string;
  /** Miejscownik („Delcie”). */
  readonly locative: string;
  /** Mianownik („Delta”). */
  readonly nominative: string;
  /** Wołacz („Delto”). */
  readonly vocative: string;
}

/** Karta sieciowa (bez adresów IP i MAC). */
export interface NetDiag {
  readonly description: string | null;
  /** `wifi`, `ethernet`, `other`. */
  readonly kind: string;
  /** Szybkość łącza (Mb/s). */
  readonly link_mbps: number | null;
  readonly name: string;
  /** Połączona. */
  readonly up: boolean;
}

/** Nowe zadanie od użytkownika (węzeł DAG: `after` = poprzedniczki, `parent_id` = delegacja). */
export interface NewTaskInput {
  readonly after: readonly string[];
  readonly agent: string | null;
  readonly goal: string;
  readonly parent_id: string | null;
  readonly session_id: string | null;
  readonly title: string;
}

/** Manifest paczki. */
export interface PackageManifestSummary {
  readonly app_version: string;
  readonly created_at: string;
  readonly encrypted: boolean;
  readonly schema_version: string;
  readonly source_machine: string;
}

/** Panel prawy. */
export type PanelId = 'agents' | 'timeline' | 'files' | 'memory' | 'screen' | 'voice' | 'tasks';

/** Uprawnienia (PLAN §8.3). */
export interface PermissionsState {
  readonly global: AutonomyLevel;
  readonly hello_enabled: boolean;
  /** Włączony „Max dostęp” (plan 50 §8.1); `None` — wyłączony albo Broker niepodłączony. */
  readonly max_access?: MaxAccess | null;
  readonly session: AutonomyLevel | null;
}

/** Zapisany lub wbudowany pipeline. */
export interface PipelineDef {
  /** Wbudowany wzorzec (nieedytowalny; zapis pod tą samą nazwą tworzy własną wersję). */
  readonly builtin: boolean;
  /** Krótki opis do czego służy. */
  readonly description: string;
  /** Unikalna nazwa (klucz). */
  readonly name: string;
  /** Etapy wykonywane po kolei (1–6). */
  readonly stages: readonly PipelineStage[];
}

/** Etap pipeline'u. */
export interface PipelineStage {
  /** Zadanie etapu; może zawierać `{{input}}` (wejście) i `{{prev}}` (wynik poprzedniego etapu). */
  readonly goal: string;
  /** Opcjonalny model `dostawca:model`; pusty — dobór Routera. */
  readonly model: string | null;
  /** Opcjonalna agentka (`delta`, `gama`, `beta`); pusta — według roli. */
  readonly persona: string | null;
  /** Rola wykonawcza (`coder`, `operator`, `researcher`, `critic`, `thinker`, `writer`, `keeper`). */
  readonly role: string;
  /** Krótka nazwa etapu pokazywana w turze i w Replay (pusta = z roli). */
  readonly title: string;
}

/** Zadeklarowana zdolność (`fs.read`, `fs.write`, `net.egress`) z zakresem. */
export interface PluginCapabilityView {
  readonly family: string;
  readonly scope: string;
}

/** Wersja wtyczki w bibliotece. */
export interface PluginInfo {
  readonly author: string;
  readonly capabilities: readonly PluginCapabilityView[];
  readonly decided_at: string | null;
  readonly description: string;
  readonly id: string;
  readonly limits: PluginLimitsView;
  readonly origin: PluginOrigin;
  readonly proposed_at: string;
  /** Propozycja R2 (tylko dla wersji czekającej na zatwierdzenie). */
  readonly r2: PluginR2View | null;
  readonly review_hash: string;
  /** Zapis plików albo sieć — narzędzia nieodwracalne, niedostępne rolom tylko do odczytu. */
  readonly side_effects: boolean;
  readonly state: PluginStateView;
  readonly tools: readonly PluginToolView[];
  readonly version: string;
  readonly wasm_sha256: string;
}

/** Wynik kontroli modułu przed propozycją (bez instalacji). */
export interface PluginInspection {
  readonly bytes: number;
  readonly error: string | null;
  readonly ok: boolean;
  readonly wasm_sha256: string;
}

/** Limity piaskownicy jednego wywołania. */
export interface PluginLimitsView {
  readonly fuel_per_call: number;
  readonly max_host_calls: number;
  readonly max_input_bytes: number;
  readonly max_output_bytes: number;
  readonly memory_mib: number;
  readonly wall_ms: number;
}

/** Pochodzenie wtyczki. */
export type PluginOrigin = 'user' | 'improver' | 'import' | 'external';

/** Ostatni problem wtyczki (bez treści wejścia/wyjścia). */
export interface PluginProblem {
  readonly at: string;
  readonly detail: string;
  readonly kind: PluginProblemKind;
  readonly plugin: string;
  readonly version: string;
}

/** Rodzaj problemu wtyczki (dla Diagnosty i listy błędów). */
export type PluginProblemKind = 'trapped' | 'load_failed';

/** Zmiana pierścienia R2 dla Ulepszacza (klucz `plugins.<id>.version` = hash przejrzanej wersji). */
export interface PluginR2View {
  readonly added_capabilities: readonly string[];
  readonly from_version: string | null;
  readonly key: string;
  readonly value: string;
}

/** Stan wersji wtyczki. */
export type PluginStateView = 'proposed' | 'installed' | 'disabled' | 'rejected' | 'superseded';

/** Narzędzie wtyczki widziane przez agentki (`plugin_<nazwa>`). */
export interface PluginToolView {
  readonly description: string;
  readonly mutating: boolean;
  readonly name: string;
  readonly title: string;
}

/** Biblioteka wtyczek. */
export interface PluginsView {
  /** Wtyczki działają (Broker i dziennik cofania podłączone, magazyn otwarty). */
  readonly available: boolean;
  readonly plugins: readonly PluginInfo[];
  readonly problems: readonly PluginProblem[];
  readonly unavailable_reason: string | null;
}

/** Zmiana ceny: nowa pozycja i cena sprzed zmiany (`None` — nowy model). */
export interface PriceChangeRow {
  readonly after: PriceRow;
  readonly before: PriceRow | null;
}

/** Cena modelu (USD za 1 mln tokenów). */
export interface PriceRow {
  readonly cache_read: number | null;
  readonly cache_write: number | null;
  readonly input: number;
  readonly model: string;
  readonly note: string | null;
  readonly output: number;
  readonly provider: string;
  readonly source: string;
  /** Cena z pliku użytkownika (zatwierdzona aktualizacja), nie z wydania. */
  readonly user: boolean;
}

/** Cennik obowiązujący i — po `pricing_apply` — zmiany (podgląd albo zapisane). */
export interface PricingView {
  readonly changes: readonly PriceChangeRow[];
  /** Dzień ostatniego sprawdzenia cen. */
  readonly checked: string;
  /** Plik cennika użytkownika. */
  readonly file: string;
  readonly rows: readonly PriceRow[];
  /** Czy zmiany zapisano (`false` — tylko podgląd). */
  readonly saved: boolean;
}

/** Projekt z katalogiem roboczym i liczbą rozmów. */
export interface ProjectInfo {
  readonly name: string;
  readonly session_count: number;
  readonly workdir: string | null;
}

/** Projekt (folder sesji). */
export interface ProjectRef {
  readonly id: string;
  readonly name: string;
}

/** Szablon promptu: tekst ze zmiennymi `{{nazwa}}` do uzupełnienia przed wstawieniem. */
export interface PromptTemplate {
  /** Identyfikator (pusty przy dodawaniu — nadaje rdzeń). */
  readonly id: string;
  readonly name: string;
  /** Treść ze zmiennymi `{{nazwa}}`. */
  readonly text: string;
}

/** Wpis katalogu dostawców. */
export interface ProviderInfo {
  readonly auth: AuthKind;
  readonly compat: CompatKind;
  readonly compliance_status: ComplianceStatus;
  readonly display_name: string;
  readonly id: string;
  readonly jurisdiction: string;
  readonly kind: ProviderKind;
  readonly needs_base_url: boolean;
  readonly privacy_tag: string;
  readonly terms_url: string | null;
}

/** Rodzaj usług dostawcy. */
export type ProviderKind = 'chat' | 'stt' | 'tts' | 'multi';

/** Uwaga o jakości: zalecenie albo metoda pomiaru według normy (bez deklaracji certyfikacji). */
export interface QualityNote {
  readonly aspect: LocalizedText;
  /** Norma albo metoda (np. `ITU-T P.800 / P.808`). */
  readonly standard: string;
  readonly text: LocalizedText;
}

/** Wynik Szybkiego pytania. */
export interface QuickAskResult {
  readonly assistant_turn_id: string | null;
  readonly session_id: string;
  readonly user_turn_id: string;
}

/** Widok strony. */
export interface QuickCommandsView {
  readonly builtins: readonly BuiltinCommand[];
  readonly commands: readonly CustomCommand[];
  /** Polecenia bez modelu włączone (`sessions.local_commands`). */
  readonly enabled: boolean;
}

/** Tryb dopasowania frazy własnego polecenia. */
export type QuickMatchMode = 'exact' | 'starts' | 'contains' | 'regex';

/** Kto odpowie na wiadomość. */
export type QuickSource = 'custom' | 'builtin' | 'quiz' | 'model' | 'disabled';

/** Wynik testera. */
export interface QuickTest {
  /** Nazwa polecenia (własne) albo rodzaj (wbudowane). */
  readonly name: string | null;
  /** Odpowiedź, którą dostanie użytkownik. */
  readonly reply: string | null;
  readonly source: QuickSource;
}

/** Limit zapytań u dostawcy. */
export interface RateLimitInfo {
  readonly provider: string;
  readonly resets_at: string;
}

/** Ocena tury. */
export type Rating = 'up' | 'down';

/** Treść plików konfiguracji. */
export interface RawConfig {
  /** Nakładka tej maszyny. */
  readonly machine: string;
  readonly machine_path: string;
  /** Problemy z ostatniego wczytania plików. */
  readonly problems: readonly string[];
  /** `shared.toml` (wspólny dla maszyn). */
  readonly shared: string;
  readonly shared_path: string;
}

/** Akcja czytania na głos. */
export type ReadAction =
  | {
      readonly kind: 'start';
      readonly source: ReadSource;
    }
  | {
      readonly control: ReadControlAction;
      readonly kind: 'control';
    }
  | {
      readonly kind: 'set_rate';
      readonly rate: number;
    };

/** Czytanie zaznaczenia / dokumentu / schowka głosem bieżącej agentki. */
export interface ReadAloudView {
  readonly agent: string;
  readonly app: string | null;
  readonly index: number;
  readonly queued: number;
  readonly rate: number;
  readonly reason: LocalizedText | null;
  readonly segments: number;
  readonly shortcut: string;
  readonly state: ReadStateView;
}

/** Sterowanie czytaniem. */
export type ReadControlAction =
  'pause' | 'resume' | 'next' | 'previous' | 'faster' | 'slower' | 'restart' | 'stop';

/** Skąd czytać. */
export type ReadSource = 'selection' | 'document' | 'clipboard';

/** Stan czytania na głos. */
export type ReadStateView = 'unavailable' | 'idle' | 'speaking' | 'paused';

/** Rekomendacja profilu. */
export interface RecommendationView {
  readonly hw_class: HwClass;
  readonly llm_backend: string;
  readonly tradeoffs: readonly LocalizedText[];
  readonly voice_profile: VoiceProfileId;
}

/** Przebudowa wektorów w tle (bez treści: tylko liczniki). */
export interface ReindexView {
  readonly cancelled: boolean;
  readonly databases: number;
  /** Postęp bieżącej bazy. */
  readonly done: number;
  readonly embedded: number;
  readonly embedder: string;
  readonly failed: number;
  readonly finished: boolean;
  readonly rebuilt: number;
  readonly running: boolean;
  readonly total: number;
}

/** Zakres „zapamiętaj". */
export type RememberScope = 'session' | 'project' | 'global' | 'agent';

/** Blok wyrenderowany w Rust (pulldown-cmark + ammonia). */
export interface RenderedBlock {
  readonly closed: boolean;
  readonly html_sanitized: string;
  readonly index: number;
  readonly kind: BlockKind;
  readonly lang: string | null;
}

/** Rodzaj kroku w Replay. */
export type ReplayKind = 'plan' | 'think' | 'tool' | 'verify' | 'steer';

/** Stan kroku w Replay. */
export type ReplayStatus =
  'running' | 'ok' | 'denied' | 'needs_confirmation' | 'failed' | 'cancelled' | 'waiting_approval';

/** Krok przebiegu (Replay: krok, narzędzie, wejście/wyjście w skrócie, status, czas, „Cofnij"). */
export interface ReplayStep {
  readonly approval_id: string | null;
  readonly at_ms: number;
  readonly duration_ms: number | null;
  readonly id: string;
  readonly input: string;
  readonly intent: ToolIntent | null;
  readonly kind: ReplayKind;
  readonly n: number;
  readonly output: string;
  readonly status: ReplayStatus;
  readonly title: string;
  readonly tool: string | null;
  readonly undo_token: string | null;
  readonly undone: boolean;
  readonly untrusted: boolean;
}

/** Poziom ryzyka akcji. */
export type RiskLevel = 'low' | 'medium' | 'high';

/** Ryzyko zmiany. */
export type RiskView = 'low' | 'medium' | 'high';

/** Rola agentki (zestaw zadań, narzędzi i uprawnień). */
export interface RoleDraft {
  /** Tworzy wyniki weryfikowane przez Krytyczkę. */
  readonly author: boolean;
  /** Zadania (opis dla UI). */
  readonly description: string;
  /** Identyfikator roli (`[a-z][a-z0-9-]{1,31}`). */
  readonly id: string;
  /** Klasa zadań dla routera (`conversation`, `code`, `research`…). */
  readonly model_policy: string;
  /** Nazwa (forma żeńska). */
  readonly name: string;
  /** Prompt roli (rodzaj żeński). */
  readonly prompt: string;
  /** Tylko odczyt. */
  readonly read_only: boolean;
  /** Grupy narzędzi (⊆ dozwolone przez politykę Kreatora). */
  readonly tools: readonly string[];
  /** Praca na niezaufanych źródłach w izolacji. */
  readonly untrusted_isolated: boolean;
}

/** Trudność zadania. */
export type RoutingLevel = 'easy' | 'medium' | 'hard';

/** Model w doborze: kandydat Routera z jego szczeblem. */
export interface RoutingModel {
  /** Szczebel rozpoznany automatycznie (po nazwie, lekki dostawca lokalny). */
  readonly automatic: RoutingRung;
  /** `dostawca:model`. */
  readonly candidate: string;
  /** Trasa lokalna. */
  readonly local: boolean;
  /** Model u dostawcy. */
  readonly model: string;
  /** Nazwa dostawcy („OpenAI”, „Model lokalny”). */
  readonly provider_name: string;
  /** Szczebel obowiązujący. */
  readonly rung: RoutingRung;
}

/** Podgląd: jak Router potraktuje przykładowe zadanie. */
export interface RoutingPreview {
  /** Trudność (brak — klasa bez doboru albo dobór wyłączony). */
  readonly difficulty: RoutingLevel | null;
  /** Klasa ma kolejkę ustawioną ręcznie i zostaje w jej kolejności. */
  readonly kept_queue: boolean;
  /** Kolejność prób (pierwszy odpowiada, reszta to zapas). */
  readonly order: readonly RoutingModel[];
  /** Powody oceny po ludzku. */
  readonly reasons: string;
  /** Wynik oceny 0–100. */
  readonly score: number | null;
  /** Szczebel startowy. */
  readonly start: RoutingRung | null;
}

/** Szczebel modelu. */
export type RoutingRung = 'light' | 'standard' | 'strong';

/** Strategia progów trudności. */
export type RoutingStrategy = 'economy' | 'balanced' | 'quality';

/** Budżety przebiegu (`[agent] default_budget`); przekroczenie = czyste zatrzymanie z raportem. */
export interface RunBudget {
  /** Maksymalny koszt (mikro-USD); `None` = bez limitu (np. model lokalny). */
  readonly max_cost_micro_usd: number | null;
  /** Maksymalna liczba kroków atomowych (tura modelu albo wywołanie narzędzia). */
  readonly max_steps: number;
  /** Maksymalna suma tokenów (wejście + wyjście). */
  readonly max_tokens: number;
  /** Maksymalna liczba wywołań narzędzi w jednej turze modelu (v0: sekwencyjnie). */
  readonly max_tool_calls_per_turn: number;
  /** Maksymalny czas przebiegu (ms, łącznie po wznowieniach). */
  readonly max_wall_ms: number;
}

/** Budżety przebiegu (z Ustawień → Agentki). */
export interface RunBudgetView {
  readonly max_cost: Money | null;
  readonly max_minutes: number;
  readonly max_steps: number;
}

/** Stan przebiegu agentki. */
export type RunState =
  | 'running'
  | 'waiting_approval'
  | 'paused'
  | 'completed'
  | 'cancelled'
  | 'failed'
  | 'budget_exceeded'
  | 'loop_detected'
  | 'refused';

/** Zużycie przebiegu. */
export interface RunUsage {
  readonly cost: Money;
  readonly elapsed_ms: number;
  readonly input_tokens: number;
  readonly output_tokens: number;
  readonly steps: number;
  readonly tool_calls: number;
}

/** Szybka rozmowa w chmurze (speech-to-speech) — dziś tylko kontrakt i atrapa. */
export interface S2sView {
  readonly available: boolean;
  readonly reason: LocalizedText;
}

/** Jakość nagranej frazy rejestracji. */
export type SampleQuality = 'good' | 'too_short' | 'too_quiet' | 'inconsistent' | 'failed';

export type SecretInput = string;

/** Opcja listy wyboru. */
export interface SelectOption {
  readonly label: LocalizedText;
  readonly value: string;
}

/** Wynik „Sprawdź silniki” (bez mikrofonu i bez odtwarzania). */
export interface SelfTestView {
  readonly stt: EngineCheckView;
  readonly tts: EngineCheckView;
  readonly turn: EngineCheckView;
}

/** Opcje wysłania wiadomości. */
export interface SendOptions {
  readonly addressed_to: AgentId | null;
  /** Identyfikatory załączników przygotowanych w composerze (`attachments_*`). */
  readonly attachments?: readonly string[];
  readonly parent_id: string | null;
  readonly profile: ModelProfile | null;
  readonly text: string;
}

/** Wynik wysłania. */
export interface SendResult {
  readonly assistant_turn_id: string | null;
  readonly user_turn_id: string;
}

/**
 * Karta rozmowy z profilem (P4, widok „Karty” panelu Sesje): dane spoza [`SessionSummary`]
 * liczone z historii — agentki, ostatnia wiadomość jako czysty tekst, model i koszt.
 */
export interface SessionCard {
  /** Agentki, które odpowiadały (bez odpowiedzi — obsada sesji); bez powtórzeń, do 4. */
  readonly agents: readonly string[];
  /** Łączny koszt rozmowy (USD, centy w górę); `None` — brak kosztu (np. model lokalny). */
  readonly cost: Money | null;
  /** Model ostatniej odpowiedzi. */
  readonly model: string | null;
  readonly session_id: string;
  /** Ostatnia widoczna wiadomość jako czysty tekst (bez Markdownu i HTML, do 160 znaków). */
  readonly snippet: string | null;
  /** Autor ostatniej wiadomości: `user` albo identyfikator agentki. */
  readonly snippet_author: string | null;
}

/** Otwarte panele sesji. */
export interface SessionPanels {
  readonly left_open: boolean;
  readonly right_open: boolean;
  readonly right_tab: PanelId;
}

/** Trafienie wyszukiwania pełnotekstowego. */
export interface SessionSearchHit {
  readonly session_id: string;
  readonly snippet: string;
  readonly title: string;
  readonly turn_id: string | null;
}

/** Pozycja listy sesji. */
export interface SessionSummary {
  readonly archived: boolean;
  readonly autonomy: AutonomyLevel;
  /** Czas założenia rozmowy. */
  readonly created_at: string;
  readonly id: string;
  readonly pinned: boolean;
  readonly profile: ModelProfile;
  readonly project: ProjectRef | null;
  readonly tags: readonly string[];
  readonly title: string;
  /** Liczba tur (archiwum: statystyka rozmowy). */
  readonly turn_count: number;
  readonly unread: boolean;
  readonly updated_at: string;
  readonly working: boolean;
}

/** Szablon nowej sesji. */
export type SessionTemplate = 'empty' | 'coding' | 'research' | 'voice' | 'admin';

/** Katalog roboczy sesji (zakres narzędzi agentek). */
export interface SessionWorkdir {
  readonly default_path: string;
  readonly path: string | null;
}

/** Kontrolka ustawienia. */
export type SettingControl =
  | {
      readonly kind: 'toggle';
    }
  | {
      readonly kind: 'select';
      readonly options: readonly SelectOption[];
    }
  | {
      readonly kind: 'number';
      readonly max: number;
      readonly min: number;
      readonly step: number;
      readonly unit: string | null;
    }
  | {
      readonly kind: 'text';
    };

/** Definicja ustawienia. */
export interface SettingDef {
  readonly control: SettingControl;
  readonly default: SettingValue;
  readonly description: LocalizedText;
  readonly key: string;
  readonly label: LocalizedText;
  readonly scope: SettingScope;
}

/** Zakres ustawienia. */
export type SettingScope = 'global' | 'session' | 'agent' | 'machine';

/** Wartość ustawienia (`boolean | number | string`); liczba zachowuje postać całkowitą/ułamkową. */
export type SettingValue = boolean | number | string;

/** Strona z własnym widokiem. */
export type SettingsCustomPage =
  | 'providers'
  | 'costs'
  | 'shortcuts'
  | 'transfer'
  | 'permissions'
  | 'devices'
  | 'voice'
  | 'api'
  | 'connections'
  | 'router'
  | 'bridges'
  | 'cameras'
  | 'memory'
  | 'triggers'
  | 'marshal'
  | 'skills'
  | 'builder'
  | 'computer'
  | 'health'
  | 'updates'
  | 'about'
  | 'plugins'
  | 'mcp'
  | 'diag'
  | 'modules'
  | 'advanced'
  | 'commands'
  | 'templates'
  | 'help'
  | 'models';

/** Strona ustawień (drzewo §15). */
export interface SettingsPageDef {
  readonly custom: SettingsCustomPage | null;
  readonly id: string;
  readonly label: LocalizedText;
  readonly settings: readonly SettingDef[];
  readonly upcoming: readonly LocalizedText[];
  readonly wave: number;
}

/** Pozycja wspólnej kolejki: model wybranego konta. */
export interface SharedQueueEntry {
  readonly account_id: string;
  readonly model: string;
}

/** Wynik importu paczki umiejętności (zawsze propozycje, nigdy instalacja). */
export interface SkillImportResult {
  readonly proposed: readonly SkillInfo[];
  readonly skipped: readonly string[];
}

/** Wersja umiejętności w bibliotece. */
export interface SkillInfo {
  readonly decided_at: string | null;
  readonly description: string;
  readonly findings: readonly string[];
  readonly hash: string;
  readonly id: string;
  readonly keywords: readonly string[];
  readonly name: string;
  readonly origin: SkillOrigin;
  readonly parameters: unknown;
  readonly proposed_at: string;
  readonly required_capabilities: readonly string[];
  readonly required_tools: readonly string[];
  readonly state: SkillStateView;
  readonly trusted: boolean;
  readonly version: string;
}

/** Pochodzenie umiejętności. */
export type SkillOrigin = 'memory' | 'user' | 'own_package' | 'external';

/** Przegląd wersji przed zatwierdzeniem: diff względem zainstalowanej + hash do potwierdzenia. */
export interface SkillReview {
  readonly diff: readonly DiffLine[];
  readonly previous_version: string | null;
  readonly skill: SkillInfo;
}

/** Stan wersji umiejętności. */
export type SkillStateView =
  'proposed' | 'quarantined' | 'installed' | 'rejected' | 'disabled' | 'superseded';

/** Pełny zapis parametrów. */
export interface Snapshot {
  /** RFC 3339. */
  readonly at: string;
  readonly battery: BatteryDiag | null;
  readonly cpu: CpuDiag;
  readonly disks: readonly DiskDiag[];
  readonly events: EventSummary;
  /** Części, których nie udało się odczytać (z powodem) — zapis jest wtedy niepełny. */
  readonly gaps: readonly string[];
  readonly gpus: readonly GpuDiag[];
  readonly machine: MachineDiag;
  readonly memory: MemDiag;
  readonly network: readonly NetDiag[];
  readonly thermal: readonly ThermalDiag[];
  readonly volumes: readonly VolumeDiag[];
}

/** Akcja kreatora rejestracji i weryfikacji właściciela. */
export type SpeakerAction =
  | {
      readonly kind: 'begin';
    }
  | {
      readonly kind: 'record_start';
    }
  | {
      readonly kind: 'record_stop';
    }
  | {
      readonly kind: 'finish';
    }
  | {
      readonly kind: 'cancel';
    }
  | {
      readonly kind: 'delete';
    }
  | {
      readonly kind: 'set_required';
      readonly required: boolean;
    };

/** Wynik weryfikacji ostatniej tury głosowej (wynik w ‰, bez embeddingu). */
export interface SpeakerCheckView {
  readonly decision: SpeakerDecisionView;
  readonly score_permille: number | null;
}

/** Decyzja weryfikacji mówcy ostatniej tury głosowej. */
export type SpeakerDecisionView = 'verified' | 'likely' | 'rejected' | 'not_checked';

/** Stan rejestracji głosu właściciela. */
export type SpeakerState = 'unavailable' | 'not_enrolled' | 'enrolling' | 'enrolled';

/** Weryfikacja właściciela („Rozpoznawanie mojego głosu”). */
export interface SpeakerView {
  readonly done: number;
  readonly last_check: SpeakerCheckView | null;
  readonly last_sample: EnrollSampleView | null;
  readonly needed: number;
  readonly prompts: readonly LocalizedText[];
  readonly reason: LocalizedText | null;
  readonly recording: boolean;
  readonly required_for_risky: boolean;
  readonly state: SpeakerState;
}

/** Powód zakończenia strumienia. */
export type StopReason = 'end' | 'refusal' | 'tool_use' | 'max_tokens' | 'cancelled';

/** Silnik rozpoznawania mowy. */
export type SttEngineKind = 'qwen_asr' | 'whisper' | 'none';

/** Rozpoznawanie mowy. */
export interface SttEngineView {
  /** Silnik główny. */
  readonly engine: SttEngineKind;
  /** Silnik zapasowy (gdy główny ma strażnika). */
  readonly fallback: SttEngineKind | null;
  /** Silnik główny liczy na karcie graficznej. */
  readonly gpu: boolean;
  /** Stan strażnika (`None` — rozmowa ani dyktowanie jeszcze nie ruszyły albo strażnika nie ma). */
  readonly guard: SttGuardView | null;
  /** Pozycje katalogu, których pobranie włączy lepszy silnik (pusto — lepszego nie ma). */
  readonly missing: readonly string[];
}

/** Strażnik rozpoznawania: bezpiecznik i skrót telemetrii z ostatnich wypowiedzi. */
export interface SttGuardView {
  readonly breaker: BreakerStateView;
  /** Awarie z rzędu. */
  readonly failures: number;
  /** Próg odstawienia silnika. */
  readonly failures_to_open: number;
  /** Ile z nich rozpoznał silnik zapasowy. */
  readonly fallbacks: number;
  /** Ile wyników miało zastrzeżenie strażnika. */
  readonly flagged: number;
  /** Mediana czasu rozpoznania (ms). */
  readonly median_ms: number | null;
  /** Ile razy bezpiecznik zadziałał od startu rozmowy. */
  readonly opened_count: number;
  /** 95. centyl czasu rozpoznania (ms). */
  readonly p95_ms: number | null;
  /** Sekundy do następnej próby (gdy otwarty). */
  readonly retry_in_s: number | null;
  /** Liczba wypowiedzi w telemetrii. */
  readonly samples: number;
}

/** Stan systemu (PLAN §14.4). */
export interface SystemStatus {
  readonly disk: DiskInfo;
  readonly keys_configured: boolean;
  readonly mic: MicAvailability;
  readonly online: boolean;
  readonly profile: ModelProfile;
  readonly queued_messages: number;
  readonly rate_limit: RateLimitInfo | null;
}

/** Klasa priorytetu. */
export type TaskClassKind = 'user' | 'agent' | 'background';

/** Krawędź DAG. */
export interface TaskDep {
  /** `succeeded`, `failed`, `finished`, `output_equals`. */
  readonly condition: string;
  readonly task_id: string;
}

/** Zadanie w panelu. */
export interface TaskInfo {
  readonly agent: string | null;
  /** `alfa`, `role:coder`, `any`, `system:<usługa>`. */
  readonly assignee: string;
  readonly attempt: number;
  /** Powód czekania (PL), błąd albo przyczyna zakończenia. */
  readonly blocked: string | null;
  readonly class: TaskClassKind;
  readonly cost: Money;
  readonly deadline_at: string;
  readonly deps: readonly TaskDep[];
  readonly error: string | null;
  /** `agent`, `bridge:claude_code`, `bridge:codex`, `service:<usługa>`. */
  readonly executor: string;
  readonly id: string;
  readonly max_attempts: number;
  readonly max_steps: number;
  readonly origin: TaskOriginKind;
  readonly origin_detail: string | null;
  readonly parent_id: string | null;
  readonly result: TaskResultKind | null;
  readonly session_id: string | null;
  readonly state: TaskStateKind;
  readonly steps: number;
  readonly submitted_at: string;
  readonly summary: string | null;
  readonly tainted: boolean;
  readonly title: string;
}

/** Pochodzenie zadania (mosty CLI tylko z `user` albo `schedule`). */
export type TaskOriginKind = 'user' | 'agent' | 'trigger' | 'schedule' | 'improver' | 'system';

/** Wynik zakończonego zadania. */
export type TaskResultKind =
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'skipped'
  | 'expired'
  | 'budget_exceeded'
  | 'budget_blocked';

/** Stan zadania. */
export type TaskStateKind = 'pending' | 'ready' | 'running' | 'retry_wait' | 'paused' | 'done';

/** Plik wybrany do nauki: wyciągnięty tekst (podgląd przed zapisem). */
export interface TeachFile {
  readonly chars: number;
  readonly name: string;
  readonly text: string;
  readonly truncated: boolean;
}

/** Wynik: ile faktów zapisano (identyfikatory do cofnięcia przez `memory_forget`). */
export interface TeachReport {
  /** Powtórzone fragmenty pominięte. */
  readonly duplicates: number;
  readonly facts: number;
  readonly ids: readonly string[];
  readonly scope_key: string;
  /** Źródło zapisane przy faktach (np. `naucz:plik:raport.pdf#3f2a…`, `rozmowa:<sesja>`). */
  readonly source: string;
  /** Treść ucięta limitem znaków albo liczby faktów. */
  readonly truncated: boolean;
  readonly trusted: boolean;
}

/** Żądanie „Naucz Alfę” (`title` puste — nazwa pliku, tytuł rozmowy albo „Wklejony tekst”). */
export interface TeachRequest {
  readonly scope: RememberScope;
  readonly session_id: string;
  readonly source: TeachSource;
  readonly title: string | null;
}

/** Źródło wiedzy. */
export type TeachSource =
  | {
      readonly kind: 'text';
      readonly text: string;
    }
  | {
      readonly kind: 'file';
      readonly name: string;
      readonly text: string;
    }
  | {
      readonly kind: 'conversation';
    };

/** Ramka strumienia terminala — wyłącznie kanał `tauri::ipc::Channel` otwarcia (nigdy zdarzenia). */
export type TerminalFrame =
  | {
      readonly data_b64: string;
      readonly kind: 'output';
    }
  | {
      readonly code: number | null;
      readonly kind: 'exit';
    };

/** Profil terminala. */
export type TerminalProfileId = 'shell' | 'cmd' | 'claude_login' | 'codex_login' | 'agy_login';

/** Otwarty terminal (bez treści). */
export interface TerminalSession {
  readonly alive: boolean;
  readonly id: number;
  readonly pid: number;
  readonly profile: TerminalProfileId;
}

/** Raport testu konta. */
export interface TestReport {
  /** Komunikat dostawcy w oryginale (bez sekretów). */
  readonly detail?: string | null;
  readonly error: string | null;
  readonly latency_ms: number | null;
  readonly models: readonly ModelInfo[];
  readonly ok: boolean;
  /** Kod błędu dostawcy (np. `invalid_api_key`). */
  readonly provider_code?: string | null;
  /**
   * Rodzaj wyniku, gdy nie jest zwykłym sukcesem: `invalid_key`, `restricted` (klucz działa,
   * ale bez prawa do listy modeli), `no_credit`, `rate_limited`, `timeout`, `network`,
   * `unsupported`. Interfejs dobiera do niego wyjaśnienie i dalszy krok.
   */
  readonly reason?: string | null;
  /** Kod HTTP odpowiedzi dostawcy. */
  readonly status?: number | null;
}

/** Czujnik temperatury. */
export interface ThermalDiag {
  readonly celsius: number;
  readonly name: string;
}

/** Blok „myślenie" (bez treści). */
export interface ThinkingInfo {
  readonly active: boolean;
  readonly duration_ms: number;
}

/** Zdarzenie osi czasu v0. */
export interface TimelineEvent {
  readonly agent: AgentId | null;
  readonly cost: Money | null;
  readonly detail: string | null;
  readonly id: string;
  readonly kind: TimelineKind;
  readonly latency_ms: number | null;
  readonly level: EventLevel;
  readonly session_id: string;
  readonly title: string;
  readonly ts: string;
  readonly turn_id: string | null;
}

/** Filtr osi czasu. */
export interface TimelineFilter {
  readonly kinds: readonly TimelineKind[];
  readonly min_level: EventLevel;
}

/** Rodzaj zdarzenia osi czasu. */
export type TimelineKind = 'model_call' | 'tool' | 'audit' | 'ui' | 'voice' | 'diagnostics';

/** Rodzaj komunikatu rdzenia. */
export type ToastKind = 'info' | 'success' | 'warning' | 'error';

/** Ikona kroku narzędzia. */
export type ToolIcon = 'file' | 'terminal' | 'search' | 'edit' | 'web' | 'memory';

/** Intencja kroku narzędzia (karta w wątku i w Replay). */
export interface ToolIntent {
  readonly command: string | null;
  readonly cwd: string | null;
  readonly kind: IntentKind;
  readonly paths: readonly string[];
  readonly shell: string | null;
  readonly title: string;
}

/** Stan kroku narzędzia. */
export type ToolStatus = 'running' | 'done' | 'error' | 'needs_confirmation';

/** Krok narzędzia. */
export interface ToolStep {
  readonly duration_ms: number | null;
  readonly icon: ToolIcon;
  readonly id: string;
  readonly intent: ToolIntent | null;
  readonly label: string;
  readonly status: ToolStatus;
  readonly undo_token: string | null;
  readonly undone: boolean;
}

/** Nowy wyzwalacz (właściciel = użytkownik). */
export interface TriggerDraft {
  readonly agent: string | null;
  readonly bridge: string | null;
  readonly goal: string;
  readonly kind: TriggerKindView;
  /** Najwyższy koszt zadania (mikro-PLN; `None` — bez limitu zadania). */
  readonly max_cost_micro_pln: number | null;
  /** Najdłuższy czas zadania w minutach (`None` — domyślnie). */
  readonly max_minutes: number | null;
  /** Najwięcej kroków zadania (`None` — domyślnie; i tak w granicach polityki Marszałka). */
  readonly max_steps: number | null;
  readonly name: string;
  /** Tylko, gdy komputer jest bezczynny (C2.7). */
  readonly only_when_idle: boolean;
  readonly respect_dnd: boolean;
  readonly title: string;
}

/** Wyzwalacz w liście. */
export interface TriggerInfo {
  readonly agent: string | null;
  /** Most CLI (`claude_code` / `codex`) — tylko harmonogram użytkownika z jawną zgodą. */
  readonly bridge: string | null;
  readonly deferred_until: string | null;
  readonly enabled: boolean;
  readonly fired: number;
  readonly goal: string;
  readonly id: string;
  readonly kind: TriggerKindView;
  readonly last_fire_at: string | null;
  /** Budżet zadania: najwyższy koszt (mikro-PLN); `None` — bez limitu zadania. */
  readonly max_cost_micro_pln: number | null;
  /** Budżet zadania: najdłuższy czas wykonania (minuty). */
  readonly max_minutes: number;
  /** Budżet zadania: najwięcej kroków. */
  readonly max_steps: number;
  readonly name: string;
  readonly next_fire_at: string | null;
  /** Zadanie startuje tylko, gdy komputer jest bezczynny (C2.7). */
  readonly only_when_idle: boolean;
  /** `user`, `agent:<persona>`, `system:<usługa>`. */
  readonly owner: string;
  readonly respect_dnd: boolean;
  readonly suppressed: number;
  readonly title: string;
  readonly tz: string;
  /** Obserwacja katalogów niedostępna na tej platformie (wyzwalacz plikowy tylko ręcznie). */
  readonly watch_unavailable: boolean;
}

/** Rodzaj wyzwalacza. */
export type TriggerKindView =
  | {
      readonly expr: string;
      readonly kind: 'cron';
    }
  | {
      readonly at: string;
      readonly kind: 'once';
    }
  | {
      readonly every_minutes: number;
      readonly kind: 'interval';
    }
  | {
      readonly dir: string;
      readonly kind: 'file_in_dir';
      readonly pattern: string | null;
    }
  | {
      readonly kind: 'new_message';
      readonly session_id: string | null;
    }
  | {
      readonly kind: 'task_finished';
      /** `succeeded`, `failed`, `any`. */
      readonly outcome: string;
      readonly task_prefix: string | null;
    }
  | {
      readonly kind: 'manual';
    };

/** Wpis dziennika uruchomień. */
export interface TriggerRunInfo {
  readonly at: string;
  /** Przyczyna (PL): „harmonogram", „ręcznie", „nowy plik: …", … */
  readonly cause: string;
  readonly detail: string | null;
  /** `submitted`, `suppressed`, `deferred`, `failed`. */
  readonly outcome: string;
  readonly task_id: string | null;
  readonly trigger_id: string;
}

/** Silnik mowy z łańcucha (kolejność = kolejność prób). */
export interface TtsEngineView {
  /** Nazwa silnika (`supertonic`, `pocket`, `piper`). */
  readonly engine: string;
  /** Pliki silnika i głosu są na miejscu. */
  readonly ready: boolean;
}

/** Tura (węzeł drzewa gałęzi, append-only). */
export interface Turn {
  readonly addressed_to: AgentId | null;
  approval: ApprovalPending | null;
  /** Załączniki tury (artefakty sesji; pole pomijane, gdy brak). */
  readonly attachments?: readonly TurnAttachment[];
  readonly author: Author;
  blocks: RenderedBlock[];
  readonly continues: string | null;
  readonly created_at: string;
  error: TurnError | null;
  /** Usłyszany prefiks przerwanej odpowiedzi głosowej (tekst; `None` = wysłuchana w całości). */
  heard_prefix: string | null;
  readonly id: string;
  readonly parent_id: string | null;
  readonly role_id: string | null;
  readonly session_id: string;
  status: TurnStatus;
  text: string;
  thinking: ThinkingInfo | null;
  tools: ToolStep[];
  truncated: boolean;
  usage: TurnUsage | null;
}

/** Adnotacje widoku (osobne rekordy, nie zmieniają tury). */
export interface TurnAnnotation {
  readonly hidden: boolean;
  readonly rating: Rating | null;
}

/** Załącznik wysłanej tury (artefakt sesji). */
export interface TurnAttachment {
  readonly artifact_id: string | null;
  readonly bytes: number | null;
  readonly kind: AttachmentKind;
  readonly mime: string;
  readonly name: string;
}

/** Co ocenia koniec wypowiedzi. */
export type TurnEngineKind = 'smart_turn' | 'text';

/** Koniec wypowiedzi. */
export interface TurnEngineView {
  readonly engine: TurnEngineKind;
  /** Pozycje katalogu, których pobranie włączy Smart Turn (pusto — już działa). */
  readonly missing: readonly string[];
  /** Liczniki modelu (`None` — model jeszcze niczego nie oceniał). */
  readonly stats: TurnStatsView | null;
}

/** Błąd tury. */
export interface TurnError {
  readonly code: TurnErrorCode;
  readonly message: string;
  readonly provider: string | null;
  readonly retry_at: string | null;
}

/** Kod błędu tury. */
export type TurnErrorCode =
  | 'offline'
  | 'rate_limited'
  | 'no_keys'
  | 'provider'
  | 'context_overflow'
  | 'budget_blocked'
  | 'task_paused';

/** Praca modelu końca wypowiedzi od startu programu. */
export interface TurnStatsView {
  /** Oceny nieudane (odpowiedziała ocena z tekstu). */
  readonly failed: number;
  /** Czas ostatniej oceny (ms). */
  readonly last_ms: number | null;
  /** Ostatnia pewność końca wypowiedzi (%). */
  readonly last_pct: number | null;
  /** Udane oceny modelu. */
  readonly scored: number;
}

/** Stan tury w widoku. */
export type TurnStatus = 'queued' | 'streaming' | 'complete' | 'cancelled' | 'error';

/** Zużycie i koszt tury. */
export interface TurnUsage {
  readonly cost: Money;
  /**
   * Koszt dokładny (mikro-PLN): tania tura kosztuje ułamek grosza, a `cost` jest w groszach
   * (0) — interfejs pokazuje wtedy „< 0,01 zł”. `None` — brak ceny modelu w cenniku: koszt
   * nieznany (interfejs nie udaje „0,00 zł”).
   */
  readonly cost_micro_pln: number | null;
  readonly input_tokens: number;
  readonly latency_ms: number;
  readonly model: string;
  readonly output_tokens: number;
  readonly provider: string;
}

/** Całe drzewo tur sesji + adnotacje. */
export interface TurnsSnapshot {
  readonly annotations: Readonly<Record<string, TurnAnnotation>>;
  readonly turns: readonly Turn[];
}

/** Bilet cofnięcia usunięcia (10 s). */
export interface UndoTicket {
  readonly expires_at: string;
  readonly token: string;
}

/** Kanał aktualizacji. */
export type UpdateChannel = 'stable' | 'beta';

/** Tryb aktualizacji (`updates.mode`). */
export type UpdateMode = 'auto' | 'ask' | 'manual';

/** Etap aktualizacji (`updater-contract::UpdatePhase`). */
export type UpdatePhase =
  | 'disabled'
  | 'idle'
  | 'checking'
  | 'up_to_date'
  | 'available'
  | 'downloading'
  | 'verifying'
  | 'installing'
  | 'ready'
  | 'failed';

/** Postęp pobierania (także częściowy plik czekający na wznowienie). */
export interface UpdateProgress {
  readonly downloaded: number;
  readonly resumed: boolean;
  readonly total: number | null;
}

/** Znalezione wydanie. */
export interface UpdateRelease {
  /** „Co nowego” z manifestu — UI pokazuje jako zwykły tekst. */
  readonly notes: string;
  readonly version: string;
}

/** Stan aktualizacji dla „Ustawienia → Aktualizacje” i banera restartu. */
export interface UpdatesView {
  readonly available: UpdateRelease | null;
  readonly channel: UpdateChannel;
  readonly current: string;
  readonly error: string | null;
  readonly last_check: string | null;
  readonly mode: UpdateMode;
  readonly phase: UpdatePhase;
  /** Cel „Przywróć poprzednią wersję”. */
  readonly previous: string | null;
  readonly progress: UpdateProgress | null;
  /** Wersja aktywna od następnego uruchomienia (aktualizacja albo przywrócenie). */
  readonly ready: string | null;
  /** Dlaczego teraz nie można uruchomić ponownie (zadanie agentki, rozmowa głosowa). */
  readonly restart_blocked: LocalizedText | null;
}

/** Wynik testu. */
export type Verdict = 'pass' | 'warn' | 'fail' | 'skipped';

/** Głos v0: mówczyni bazowa wbudowanego silnika + wysokość i tempo (bez kluczy, bez klonów). */
export interface VoiceDraft {
  /** Mówczyni bazowa (`pl-f1`, `pl-f2`, zapas Piper). */
  readonly base: string;
  /** Prompt voice design (bez „girl/cute/child”). */
  readonly design_prompt: string;
  /** Postrzegany wiek (18–25). */
  readonly perceived_age: number;
  /** Wysokość (0,7–1,4). */
  readonly pitch: number;
  /** Tempo (0,6–1,6). */
  readonly rate: number;
  /** Barwa (opis). */
  readonly timbre: string;
}

/** Silniki rozmowy. */
export interface VoiceEnginesView {
  /** Wynik ostatniej próby silników (`None` — jeszcze nie była uruchamiana). */
  readonly self_test: SelfTestView | null;
  readonly stt: SttEngineView;
  readonly tts: readonly TtsEngineView[];
  readonly turn: TurnEngineView;
}

/** Głos rozszerzony (panel Głos, Ustawienia → Głos). */
export interface VoiceFeatures {
  readonly dictation: DictationView;
  /** Silniki rozmowy: rozpoznawanie ze strażnikiem, koniec wypowiedzi, głosy. */
  readonly engines: VoiceEnginesView;
  readonly read: ReadAloudView;
  readonly s2s: S2sView;
  readonly speaker: SpeakerView;
  readonly wake: WakeWordsView;
}

/** Głos (barwa modelu mowy) z jego warunkami. */
export interface VoiceLicense {
  readonly commercial: CommercialUse;
  /** Identyfikator głosu w silniku (np. `F1`, `pl_PL-gosia-medium`). */
  readonly id: string;
  /** Licencja głosu (zwykle ta sama co wag modelu). */
  readonly license: string;
  readonly name: LocalizedText;
  readonly notes: LocalizedText;
  /** Skąd pochodzi głos (barwa wbudowana, nagrania wzorcowe). */
  readonly origin: LocalizedText;
}

/** Sposób włączania mikrofonu. */
export type VoiceMode = 'toggle' | 'ptt';

/** Pigułka głosowa. */
export interface VoicePillState {
  readonly agent: AgentId;
  readonly level: number;
  readonly mic: MicState;
  /** Transkrypt częściowy wypowiedzi użytkownika (szary w UI). */
  readonly partial: string | null;
  /** Kto mówi teraz. */
  readonly speaker: VoiceSpeaker;
}

/** Profil głosu A–D. */
export type VoiceProfileId = 'A' | 'B' | 'C' | 'D';

/** Kto mówi (pigułka). */
export type VoiceSpeaker = 'nobody' | 'user' | 'agent';

/** Stan trybu głosowego. */
export type VoiceState = 'unavailable' | 'off' | 'active';

/** Stan potoku głosu (pasek tytułu, Ustawienia → Głos). */
export interface VoiceStatus {
  readonly agent: AgentId;
  readonly missing: readonly string[];
  readonly mode: VoiceMode;
  readonly muted: boolean;
  readonly reason: LocalizedText | null;
  readonly state: VoiceState;
}

/** Wolumin (litera dysku). */
export interface VolumeDiag {
  readonly file_system: string | null;
  readonly free_mb: number;
  readonly label: string | null;
  /** Litera z dwukropkiem (`C:`). */
  readonly letter: string;
  readonly size_mb: number;
  /** Wolumin systemowy. */
  readonly system: boolean;
}

/** Akcja słów wywoławczych. */
export type WakeAction =
  | {
      readonly accept_risk: boolean;
      readonly enabled: boolean;
      readonly kind: 'configure';
      readonly owner_gate: boolean;
    }
  | {
      readonly kind: 'test';
      readonly on: boolean;
    }
  | {
      readonly kind: 'set_dnd';
      readonly on: boolean;
    };

/** Wynik pomiaru FAR/FRR z runnera `alfa-wake-eval` (brak pomiaru — `measured = false`). */
export interface WakeCalibrationView {
  readonly far_per_day: number | null;
  readonly frr: number | null;
  readonly measured: boolean;
  readonly passes: boolean;
  readonly sufficient: boolean;
  readonly threshold: number;
}

/** Test słowa wywoławczego (wykrycia bez otwierania rozmowy). */
export interface WakeTestView {
  readonly active: boolean;
  readonly detections: number;
  readonly last_agent: string | null;
  readonly owner_rejected: number;
}

/** Stan słów wywoławczych. */
export type WakeWordsState = 'unavailable' | 'off' | 'armed' | 'listening' | 'suspended';

/** Słowa wywoławcze „Hej Alfa/Beta/Gama/Delta”. */
export interface WakeWordsView {
  readonly calibration: WakeCalibrationView;
  readonly dnd: boolean;
  readonly enabled: boolean;
  readonly owner_gate: boolean;
  readonly phrases: readonly string[];
  readonly reason: LocalizedText | null;
  readonly risk_accepted: boolean;
  readonly state: WakeWordsState;
  readonly test: WakeTestView;
}

/** „Co nowego” — raz po aktualizacji. */
export interface WhatsNew {
  /** Notatki z podpisanej paczki (`notes.md`) — UI pokazuje jako zwykły tekst. */
  readonly notes: string;
  readonly version: string;
}

/** Wybór katalogu roboczego sesji. */
export type WorkdirChoice = 'dialog' | 'default' | 'none';

/**
 * Komendy rdzenia: nazwa → argumenty (nazwy tak, jak idą z JS — camelCase) i wynik.
 * Wynik `null` to komenda bez wartości zwracanej.
 */
export interface IpcCommands {
  readonly accounts_add: {
    readonly args: { readonly input: AddAccountInput };
    readonly result: Account;
  };
  readonly accounts_assign: {
    readonly args: { readonly accountId: string; readonly assignment: AccountAssignment };
    readonly result: null;
  };
  readonly accounts_catalog: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly ProviderInfo[];
  };
  readonly accounts_import_env: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly EnvImportResult[];
  };
  readonly accounts_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly Account[];
  };
  readonly accounts_remove: {
    readonly args: { readonly accountId: string };
    readonly result: null;
  };
  readonly accounts_rotate: {
    readonly args: { readonly accountId: string; readonly secret: SecretInput };
    readonly result: TestReport;
  };
  readonly accounts_set_disabled: {
    readonly args: { readonly accountId: string; readonly disabled: boolean };
    readonly result: Account;
  };
  readonly accounts_set_enterprise: {
    readonly args: { readonly accountId: string; readonly enterprise: EnterpriseSettings };
    readonly result: Account;
  };
  readonly accounts_set_limit: {
    readonly args: {
      readonly accountId: string;
      readonly enabled: boolean;
      readonly monthly: Money;
    };
    readonly result: null;
  };
  readonly accounts_set_model: {
    readonly args: { readonly accountId: string; readonly model: string };
    readonly result: Account;
  };
  readonly accounts_set_queue: {
    readonly args: {
      readonly accountId: string;
      readonly taskClass: string;
      readonly models: readonly string[];
    };
    readonly result: Account;
  };
  readonly accounts_set_shared_queue: {
    readonly args: { readonly taskClass: string; readonly entries: readonly SharedQueueEntry[] };
    readonly result: Readonly<Record<string, readonly SharedQueueEntry[]>>;
  };
  readonly accounts_shared_queues: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: Readonly<Record<string, readonly SharedQueueEntry[]>>;
  };
  readonly accounts_suggest_queue: {
    readonly args: {
      readonly accountId: string;
      readonly taskClass: string;
      readonly strategy: string;
    };
    readonly result: readonly string[];
  };
  readonly accounts_test: {
    readonly args: { readonly accountId: string };
    readonly result: TestReport;
  };
  readonly agents_apply_cast: {
    readonly args: { readonly sessionId: string; readonly template: CastTemplateId };
    readonly result: null;
  };
  readonly agents_list: {
    readonly args: { readonly sessionId: string };
    readonly result: readonly AgentState[];
  };
  readonly agents_open_terminal: {
    readonly args: { readonly stepId: string };
    readonly result: null;
  };
  readonly agents_runs: {
    readonly args: { readonly sessionId: string };
    readonly result: readonly AgentRunDetail[];
  };
  readonly agents_set_roles: {
    readonly args: {
      readonly sessionId: string;
      readonly agent: string;
      readonly roleIds: readonly string[];
    };
    readonly result: null;
  };
  readonly agents_steer: {
    readonly args: { readonly sessionId: string; readonly text: string };
    readonly result: null;
  };
  readonly app_bootstrap: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: AppBootstrap;
  };
  readonly app_complete_onboarding: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: null;
  };
  readonly app_open_system_settings: {
    readonly args: { readonly uri: string };
    readonly result: null;
  };
  readonly app_save_layout: {
    readonly args: { readonly layout: LayoutPrefs };
    readonly result: null;
  };
  readonly app_set_active_session: {
    readonly args: { readonly sessionId: string | null };
    readonly result: null;
  };
  readonly attachments_add_camera: {
    readonly args: { readonly sessionId: string; readonly cameraId: string };
    readonly result: AttachmentsAdded;
  };
  readonly attachments_add_dropped: {
    readonly args: { readonly sessionId: string };
    readonly result: AttachmentsAdded;
  };
  readonly attachments_list: {
    readonly args: { readonly sessionId: string };
    readonly result: readonly AttachmentInfo[];
  };
  readonly attachments_paste: {
    readonly args: { readonly sessionId: string };
    readonly result: AttachmentsAdded;
  };
  readonly attachments_pick: {
    readonly args: { readonly sessionId: string };
    readonly result: AttachmentsAdded;
  };
  readonly attachments_remove: {
    readonly args: { readonly sessionId: string; readonly attachmentId: string };
    readonly result: readonly AttachmentInfo[];
  };
  readonly backups_choose_dir: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: BackupView;
  };
  readonly backups_configure: {
    readonly args: { readonly config: BackupConfig };
    readonly result: BackupView;
  };
  readonly backups_restore: { readonly args: { readonly file: string }; readonly result: string };
  readonly backups_run_now: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: BackupView;
  };
  readonly backups_set_password: {
    readonly args: { readonly password: SecretInput | null };
    readonly result: BackupView;
  };
  readonly backups_status: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: BackupView;
  };
  readonly backups_verify: {
    readonly args: { readonly file: string };
    readonly result: BackupCheck;
  };
  readonly board_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly BoardItem[];
  };
  readonly board_pin: {
    readonly args: { readonly sessionId: string; readonly turnId: string };
    readonly result: readonly BoardItem[];
  };
  readonly board_unpin: {
    readonly args: { readonly id: string };
    readonly result: readonly BoardItem[];
  };
  readonly bridges_check: {
    readonly args: { readonly bridge: string };
    readonly result: BridgeCard;
  };
  readonly bridges_list: {
    readonly args: { readonly refresh: boolean };
    readonly result: readonly BridgeCard[];
  };
  readonly bridges_mark_login: {
    readonly args: { readonly bridge: string; readonly plan: string | null };
    readonly result: BridgeCard;
  };
  readonly bridges_open_login: {
    readonly args: { readonly bridge: string };
    readonly result: BridgeLogin;
  };
  readonly bridges_pin: {
    readonly args: { readonly bridge: string; readonly version: string | null };
    readonly result: BridgeCard;
  };
  readonly bridges_set_enabled: {
    readonly args: { readonly routeId: string; readonly enabled: boolean };
    readonly result: BridgeCard;
  };
  readonly bridges_set_schedule: {
    readonly args: { readonly bridge: string; readonly perDay: number };
    readonly result: BridgeCard;
  };
  readonly broker_status: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: BrokerStatusView;
  };
  readonly builder_dry_run: {
    readonly args: { readonly draft: AgentDraft };
    readonly result: BuilderDryRun;
  };
  readonly builder_library: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly BuilderAgentInfo[];
  };
  readonly builder_policy: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: BuilderPolicyView;
  };
  readonly builder_preview: {
    readonly args: { readonly draft: AgentDraft };
    readonly result: BuilderPreview;
  };
  readonly builder_propose: {
    readonly args: { readonly description: string };
    readonly result: BuilderProposal;
  };
  readonly builder_save: {
    readonly args: { readonly draft: AgentDraft; readonly hash: string };
    readonly result: BuilderSaved;
  };
  readonly builder_voice_preview: {
    readonly args: { readonly draft: AgentDraft };
    readonly result: null;
  };
  readonly cameras_frame: {
    readonly args: { readonly id: string; readonly maxSide: number };
    readonly result: CameraFrame;
  };
  readonly cameras_refresh: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: CamerasState;
  };
  readonly cameras_rename: {
    readonly args: { readonly id: string; readonly name: string };
    readonly result: CamerasState;
  };
  readonly cameras_set_enabled: {
    readonly args: { readonly id: string; readonly enabled: boolean };
    readonly result: CamerasState;
  };
  readonly cameras_set_master: {
    readonly args: { readonly enabled: boolean };
    readonly result: CamerasState;
  };
  readonly cameras_state: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: CamerasState;
  };
  readonly canvas_render: {
    readonly args: { readonly kind: CanvasKind; readonly source: string; readonly lang: string };
    readonly result: string;
  };
  readonly compliance_routes: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: DataRoutesView;
  };
  readonly config_raw_check: { readonly args: { readonly text: string }; readonly result: number };
  readonly config_raw_get: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: RawConfig;
  };
  readonly config_raw_save: {
    readonly args: { readonly layer: string; readonly text: string };
    readonly result: RawConfig;
  };
  readonly costs_set_monthly_limit: {
    readonly args: { readonly enabled: boolean; readonly monthly: Money };
    readonly result: null;
  };
  readonly costs_summary: {
    readonly args: { readonly sessionId: string | null };
    readonly result: CostSummary;
  };
  readonly device_gpu_live: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: GpuLive;
  };
  readonly device_measure: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: DeviceProfile;
  };
  readonly device_profile: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: DeviceProfile;
  };
  readonly diag_cancel: { readonly args: Readonly<Record<string, never>>; readonly result: null };
  readonly diag_export: { readonly args: Readonly<Record<string, never>>; readonly result: string };
  readonly diag_run: {
    readonly args: { readonly tests: readonly string[] };
    readonly result: CheckRun;
  };
  readonly diag_runs: {
    readonly args: { readonly limit: number };
    readonly result: readonly CheckRun[];
  };
  readonly diag_snapshot: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: DiagView;
  };
  readonly diag_state: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: DiagView;
  };
  readonly embed_model_activate: {
    readonly args: { readonly model: string };
    readonly result: EmbedderView;
  };
  readonly evals_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: EvalsView;
  };
  readonly evals_verify: {
    readonly args: { readonly suiteId: string };
    readonly result: EvalSuiteView;
  };
  readonly events_resume: {
    readonly args: { readonly afterSeq: number; readonly epoch: string | null };
    readonly result: EventsResume;
  };
  readonly files_act: {
    readonly args: { readonly artifactId: string; readonly action: ArtifactAction };
    readonly result: null;
  };
  readonly files_list: {
    readonly args: { readonly sessionId: string };
    readonly result: readonly ArtifactInfo[];
  };
  readonly files_preview: {
    readonly args: { readonly artifactId: string };
    readonly result: ArtifactPreview;
  };
  readonly gui_desktop_grant: {
    readonly args: { readonly sessionId: string; readonly agent: string };
    readonly result: BrokerIntentResult;
  };
  readonly gui_release: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: GuiStatus;
  };
  readonly gui_screenshot: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: GuiScreenshot | null;
  };
  readonly gui_status: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: GuiStatus;
  };
  readonly gui_stop: { readonly args: Readonly<Record<string, never>>; readonly result: GuiStatus };
  readonly health_approve: {
    readonly args: { readonly repairId: number };
    readonly result: HealthView;
  };
  readonly health_reject: {
    readonly args: { readonly repairId: number };
    readonly result: HealthView;
  };
  readonly health_report: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: HealthView;
  };
  readonly health_scan: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: HealthView;
  };
  readonly health_undo: {
    readonly args: { readonly repairId: number };
    readonly result: HealthView;
  };
  readonly help_page: { readonly args: { readonly id: string }; readonly result: HelpPage };
  readonly help_search: {
    readonly args: { readonly query: string };
    readonly result: readonly HelpHit[];
  };
  readonly help_topics: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly HelpTopic[];
  };
  readonly improver_approve: {
    readonly args: { readonly proposalId: number; readonly digest: string };
    readonly result: ImproverView;
  };
  readonly improver_cycle: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ImproverView;
  };
  readonly improver_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ImproverView;
  };
  readonly improver_reject: {
    readonly args: { readonly proposalId: number };
    readonly result: ImproverView;
  };
  readonly improver_rollback: {
    readonly args: { readonly proposalId: number };
    readonly result: ImproverView;
  };
  readonly marshal_approve: {
    readonly args: { readonly proposalId: number };
    readonly result: readonly MarshalRuleInfo[];
  };
  readonly marshal_propose: {
    readonly args: { readonly text: string; readonly drafts: readonly unknown[] | null };
    readonly result: MarshalProposalInfo;
  };
  readonly marshal_reject: {
    readonly args: { readonly proposalId: number };
    readonly result: null;
  };
  readonly marshal_report: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: MarshalReport;
  };
  readonly marshal_revoke: { readonly args: { readonly ruleId: string }; readonly result: null };
  readonly marshal_state: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: MarshalState;
  };
  readonly mcp_approve: {
    readonly args: { readonly id: string; readonly tool: string; readonly fingerprint: string };
    readonly result: McpServerTools;
  };
  readonly mcp_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly McpServerView[];
  };
  readonly mcp_remove: { readonly args: { readonly id: string }; readonly result: null };
  readonly mcp_revoke: {
    readonly args: { readonly id: string; readonly tool: string };
    readonly result: McpServerTools;
  };
  readonly mcp_save: {
    readonly args: { readonly input: McpServerInput };
    readonly result: McpServerView;
  };
  readonly mcp_tools: { readonly args: { readonly id: string }; readonly result: McpServerTools };
  readonly memory_approve: {
    readonly args: { readonly entryId: string };
    readonly result: MemoryItem;
  };
  readonly memory_consolidate_now: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ConsolidationReport;
  };
  readonly memory_edit: {
    readonly args: { readonly entryId: string; readonly edit: MemoryEdit };
    readonly result: MemoryItem;
  };
  readonly memory_explain: {
    readonly args: { readonly entryId: string };
    readonly result: MemoryExplanation;
  };
  readonly memory_forget: {
    readonly args: { readonly target: MemoryForgetTarget };
    readonly result: MemoryForgetReport;
  };
  readonly memory_forget_preview: {
    readonly args: { readonly target: MemoryForgetTarget };
    readonly result: MemoryForgetPreview;
  };
  readonly memory_inspect: {
    readonly args: { readonly query: MemoryQuery };
    readonly result: MemoryPage;
  };
  readonly memory_journal: {
    readonly args: { readonly scope: string };
    readonly result: readonly MemoryJournalEntry[];
  };
  readonly memory_promote: {
    readonly args: { readonly entryId: string; readonly to: MemoryScopeRef };
    readonly result: MemoryItem;
  };
  readonly memory_scopes: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly MemoryScopeInfo[];
  };
  readonly memory_set_pinned: {
    readonly args: { readonly entryId: string; readonly pinned: boolean };
    readonly result: MemoryItem;
  };
  readonly memory_status: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: MemoryStatus;
  };
  readonly memory_teach: {
    readonly args: { readonly request: TeachRequest };
    readonly result: TeachReport;
  };
  readonly memory_teach_pick_file: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: TeachFile | null;
  };
  readonly memory_undo: {
    readonly args: { readonly scope: string; readonly changeId: string };
    readonly result: MemoryUndoResult;
  };
  readonly models_bundle_download: {
    readonly args: { readonly bundleId: string };
    readonly result: ModelBundle;
  };
  readonly models_bundle_verify: {
    readonly args: { readonly bundleId: string };
    readonly result: ModelBundle;
  };
  readonly models_bundles: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly ModelBundle[];
  };
  readonly models_cancel: {
    readonly args: { readonly itemId: string };
    readonly result: ModelItem;
  };
  readonly models_download: {
    readonly args: { readonly itemId: string };
    readonly result: ModelItem;
  };
  readonly models_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ModelsView;
  };
  readonly models_local_cancel: {
    readonly args: { readonly modelId: string | null };
    readonly result: null;
  };
  readonly models_local_download: {
    readonly args: { readonly modelId: string | null };
    readonly result: null;
  };
  readonly models_local_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly LocalModelInfo[];
  };
  readonly models_remove: {
    readonly args: { readonly itemId: string };
    readonly result: ModelItem;
  };
  readonly models_repair: {
    readonly args: { readonly itemId: string };
    readonly result: ModelItem;
  };
  readonly models_trust_hash: {
    readonly args: { readonly itemId: string; readonly hashes: Readonly<Record<string, string>> };
    readonly result: ModelItem;
  };
  readonly models_verify: {
    readonly args: { readonly itemId: string };
    readonly result: ModelItem;
  };
  readonly modules_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ModulesView;
  };
  readonly modules_set_enabled: {
    readonly args: { readonly id: string; readonly enabled: boolean };
    readonly result: ModulesView;
  };
  readonly permissions_child_lock: {
    readonly args: { readonly change: ChildLockRequest | null };
    readonly result: ChildLockState;
  };
  readonly permissions_get: {
    readonly args: { readonly sessionId: string | null };
    readonly result: PermissionsState;
  };
  readonly permissions_grants: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly GrantView[];
  };
  readonly permissions_open_approval: {
    readonly args: { readonly approvalId: string };
    readonly result: BrokerIntentResult;
  };
  readonly permissions_request_level: {
    readonly args: { readonly level: AutonomyLevel; readonly sessionId: string | null };
    readonly result: BrokerIntentResult;
  };
  readonly permissions_request_max_access: {
    readonly args: { readonly scope: MaxAccess | null };
    readonly result: BrokerIntentResult;
  };
  readonly permissions_revoke_grant: {
    readonly args: { readonly grantId: string };
    readonly result: boolean;
  };
  readonly pipelines_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly PipelineDef[];
  };
  readonly pipelines_remove: {
    readonly args: { readonly name: string };
    readonly result: readonly PipelineDef[];
  };
  readonly pipelines_save: {
    readonly args: { readonly pipeline: PipelineDef; readonly previousName: string | null };
    readonly result: readonly PipelineDef[];
  };
  readonly plugins_approve: {
    readonly args: {
      readonly pluginId: string;
      readonly version: string;
      readonly reviewedHash: string;
    };
    readonly result: PluginInfo;
  };
  readonly plugins_disable: {
    readonly args: { readonly pluginId: string };
    readonly result: PluginInfo;
  };
  readonly plugins_enable: {
    readonly args: { readonly pluginId: string; readonly reviewedHash: string };
    readonly result: PluginInfo;
  };
  readonly plugins_inspect: {
    readonly args: { readonly wasmB64: string };
    readonly result: PluginInspection;
  };
  readonly plugins_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: PluginsView;
  };
  readonly plugins_propose: {
    readonly args: { readonly manifest: unknown; readonly wasmB64: string };
    readonly result: PluginInfo;
  };
  readonly plugins_reject: {
    readonly args: { readonly pluginId: string; readonly version: string };
    readonly result: PluginInfo;
  };
  readonly plugins_remove: {
    readonly args: { readonly pluginId: string };
    readonly result: PluginsView;
  };
  readonly pricing_apply: {
    readonly args: { readonly text: string; readonly save: boolean };
    readonly result: PricingView;
  };
  readonly pricing_get: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: PricingView;
  };
  readonly projects_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly ProjectInfo[];
  };
  readonly projects_pick_dir: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: string | null;
  };
  readonly projects_remove: {
    readonly args: { readonly name: string };
    readonly result: readonly ProjectInfo[];
  };
  readonly projects_rename: {
    readonly args: { readonly oldName: string; readonly newName: string };
    readonly result: readonly ProjectInfo[];
  };
  readonly projects_save: {
    readonly args: { readonly name: string; readonly workdir: string | null };
    readonly result: readonly ProjectInfo[];
  };
  readonly prompt_templates_get: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly PromptTemplate[];
  };
  readonly prompt_templates_set: {
    readonly args: { readonly templates: readonly PromptTemplate[] };
    readonly result: readonly PromptTemplate[];
  };
  readonly quick_ask: { readonly args: { readonly text: string }; readonly result: QuickAskResult };
  readonly quick_commands_get: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: QuickCommandsView;
  };
  readonly quick_commands_set: {
    readonly args: { readonly commands: readonly CustomCommand[] };
    readonly result: QuickCommandsView;
  };
  readonly quick_commands_test: {
    readonly args: { readonly message: string };
    readonly result: QuickTest;
  };
  readonly quick_expand_to_main: {
    readonly args: { readonly sessionId: string };
    readonly result: null;
  };
  readonly quick_hide: { readonly args: Readonly<Record<string, never>>; readonly result: null };
  readonly routing_get: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ModelRouting;
  };
  readonly routing_preview: {
    readonly args: { readonly text: string; readonly taskClass: string };
    readonly result: RoutingPreview;
  };
  readonly routing_set: {
    readonly args: { readonly settings: ModelRoutingSettings };
    readonly result: ModelRouting;
  };
  readonly search_reindex_cancel: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ReindexView;
  };
  readonly search_reindex_start: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ReindexView;
  };
  readonly search_reindex_status: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ReindexView;
  };
  readonly sessions_cards: {
    readonly args: { readonly sessionIds: readonly string[] };
    readonly result: readonly SessionCard[];
  };
  readonly sessions_choose_workdir: {
    readonly args: { readonly sessionId: string; readonly choice: WorkdirChoice };
    readonly result: SessionWorkdir;
  };
  readonly sessions_create: {
    readonly args: { readonly template: SessionTemplate };
    readonly result: SessionSummary;
  };
  readonly sessions_duplicate_as_template: {
    readonly args: { readonly sessionId: string };
    readonly result: SessionSummary;
  };
  readonly sessions_export: {
    readonly args: { readonly sessionId: string };
    readonly result: ExportResult;
  };
  readonly sessions_export_conversation: {
    readonly args: {
      readonly sessionId: string;
      readonly format: ConversationFormat;
      readonly turnId: string | null;
    };
    readonly result: ExportResult;
  };
  readonly sessions_get_draft: {
    readonly args: { readonly sessionId: string };
    readonly result: string;
  };
  readonly sessions_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly SessionSummary[];
  };
  readonly sessions_mark_read: {
    readonly args: { readonly sessionId: string };
    readonly result: null;
  };
  readonly sessions_remove: {
    readonly args: { readonly sessionId: string };
    readonly result: UndoTicket;
  };
  readonly sessions_rename: {
    readonly args: { readonly sessionId: string; readonly title: string };
    readonly result: null;
  };
  readonly sessions_save_draft: {
    readonly args: { readonly sessionId: string; readonly text: string };
    readonly result: null;
  };
  readonly sessions_search: {
    readonly args: { readonly query: string };
    readonly result: readonly SessionSearchHit[];
  };
  readonly sessions_set_archived: {
    readonly args: { readonly sessionId: string; readonly archived: boolean };
    readonly result: null;
  };
  readonly sessions_set_pinned: {
    readonly args: { readonly sessionId: string; readonly pinned: boolean };
    readonly result: null;
  };
  readonly sessions_set_project: {
    readonly args: { readonly sessionId: string; readonly project: string | null };
    readonly result: null;
  };
  readonly sessions_undo_remove: {
    readonly args: { readonly token: string };
    readonly result: null;
  };
  readonly sessions_workdir: {
    readonly args: { readonly sessionId: string };
    readonly result: SessionWorkdir;
  };
  readonly settings_reset: {
    readonly args: { readonly key: string };
    readonly result: SettingValue;
  };
  readonly settings_schema: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly SettingsPageDef[];
  };
  readonly settings_set: {
    readonly args: { readonly key: string; readonly value: SettingValue };
    readonly result: null;
  };
  readonly settings_set_shortcut: {
    readonly args: { readonly actionId: string; readonly chord: string | null };
    readonly result: null;
  };
  readonly settings_values: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: Readonly<Record<string, SettingValue>>;
  };
  readonly skills_approve: {
    readonly args: { readonly skillId: string; readonly version: string; readonly hash: string };
    readonly result: SkillInfo;
  };
  readonly skills_disable: {
    readonly args: { readonly skillId: string };
    readonly result: SkillInfo;
  };
  readonly skills_export: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: ExportResult;
  };
  readonly skills_import: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: SkillImportResult;
  };
  readonly skills_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly SkillInfo[];
  };
  readonly skills_propose: {
    readonly args: { readonly skill: unknown };
    readonly result: SkillInfo;
  };
  readonly skills_reject: {
    readonly args: { readonly skillId: string; readonly version: string };
    readonly result: SkillInfo;
  };
  readonly skills_release: {
    readonly args: { readonly skillId: string; readonly version: string; readonly hash: string };
    readonly result: SkillInfo;
  };
  readonly skills_review: {
    readonly args: { readonly skillId: string; readonly version: string };
    readonly result: SkillReview;
  };
  readonly skills_run: {
    readonly args: {
      readonly skillId: string;
      readonly sessionId: string;
      readonly agent: string | null;
      readonly params: unknown;
    };
    readonly result: TaskInfo;
  };
  readonly system_clipboard_text: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: string | null;
  };
  readonly system_retry_queue: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: null;
  };
  readonly system_status: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: SystemStatus;
  };
  readonly tasks_cancel: {
    readonly args: { readonly taskId: string };
    readonly result: readonly string[];
  };
  readonly tasks_create: {
    readonly args: { readonly input: NewTaskInput };
    readonly result: TaskInfo;
  };
  readonly tasks_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly TaskInfo[];
  };
  readonly tasks_pause: { readonly args: { readonly taskId: string }; readonly result: null };
  readonly tasks_resume: { readonly args: { readonly taskId: string }; readonly result: null };
  readonly tasks_retry: { readonly args: { readonly taskId: string }; readonly result: TaskInfo };
  readonly tasks_steer: {
    readonly args: { readonly taskId: string; readonly text: string };
    readonly result: null;
  };
  readonly terminal_close: { readonly args: { readonly terminal: number }; readonly result: null };
  readonly terminal_input: {
    readonly args: { readonly terminal: number; readonly dataB64: string };
    readonly result: null;
  };
  readonly terminal_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly TerminalSession[];
  };
  readonly terminal_resize: {
    readonly args: { readonly terminal: number; readonly cols: number; readonly rows: number };
    readonly result: null;
  };
  readonly timeline_list: {
    readonly args: { readonly sessionId: string; readonly filter: TimelineFilter };
    readonly result: readonly TimelineEvent[];
  };
  readonly transfer_export: {
    readonly args: { readonly request: ExportRequest };
    readonly result: ExportResult;
  };
  readonly transfer_import: {
    readonly args: { readonly request: ImportRequest };
    readonly result: ImportResult;
  };
  readonly transfer_inspect: {
    readonly args: { readonly password: SecretInput | null; readonly handle: string | null };
    readonly result: InspectResult;
  };
  readonly transfer_rollback: {
    readonly args: { readonly snapshotId: string };
    readonly result: null;
  };
  readonly triggers_create: {
    readonly args: { readonly draft: TriggerDraft };
    readonly result: TriggerInfo;
  };
  readonly triggers_fire_now: {
    readonly args: { readonly triggerId: string };
    readonly result: TriggerRunInfo;
  };
  readonly triggers_list: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly TriggerInfo[];
  };
  readonly triggers_log: {
    readonly args: { readonly triggerId: string | null };
    readonly result: readonly TriggerRunInfo[];
  };
  readonly triggers_preview_cron: {
    readonly args: { readonly expr: string };
    readonly result: CronPreview;
  };
  readonly triggers_remove: {
    readonly args: { readonly triggerId: string };
    readonly result: null;
  };
  readonly triggers_set_enabled: {
    readonly args: { readonly triggerId: string; readonly enabled: boolean };
    readonly result: null;
  };
  readonly turns_arena_answer: {
    readonly args: { readonly sessionId: string; readonly turnId: string; readonly model: string };
    readonly result: string;
  };
  readonly turns_continue: {
    readonly args: { readonly sessionId: string; readonly turnId: string };
    readonly result: string;
  };
  readonly turns_edit_and_resend: {
    readonly args: { readonly sessionId: string; readonly turnId: string; readonly text: string };
    readonly result: SendResult;
  };
  readonly turns_list: {
    readonly args: { readonly sessionId: string };
    readonly result: TurnsSnapshot;
  };
  readonly turns_rate: {
    readonly args: { readonly turnId: string; readonly rating: Rating | null };
    readonly result: null;
  };
  readonly turns_read_aloud: { readonly args: { readonly turnId: string }; readonly result: null };
  readonly turns_regenerate: {
    readonly args: {
      readonly sessionId: string;
      readonly turnId: string;
      readonly profile: string | null;
    };
    readonly result: string;
  };
  readonly turns_remember: {
    readonly args: { readonly turnId: string; readonly scope: RememberScope };
    readonly result: null;
  };
  readonly turns_run_code: {
    readonly args: { readonly turnId: string; readonly blockIndex: number };
    readonly result: BrokerIntentResult;
  };
  readonly turns_save_code: {
    readonly args: { readonly turnId: string; readonly blockIndex: number };
    readonly result: null;
  };
  readonly turns_select_branch: {
    readonly args: { readonly sessionId: string; readonly turnId: string };
    readonly result: null;
  };
  readonly turns_send: {
    readonly args: { readonly sessionId: string; readonly options: SendOptions };
    readonly result: SendResult;
  };
  readonly turns_set_hidden: {
    readonly args: { readonly turnId: string; readonly hidden: boolean };
    readonly result: null;
  };
  readonly turns_stop: { readonly args: { readonly sessionId: string }; readonly result: null };
  readonly turns_undo_step: {
    readonly args: { readonly undoToken: string };
    readonly result: null;
  };
  readonly updates_about: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: AboutInfo;
  };
  readonly updates_cancel: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: UpdatesView;
  };
  readonly updates_check: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: UpdatesView;
  };
  readonly updates_dismiss_whats_new: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: null;
  };
  readonly updates_download: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: UpdatesView;
  };
  readonly updates_history: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly ChangeLogEntry[];
  };
  readonly updates_restart: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: null;
  };
  readonly updates_rollback: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: UpdatesView;
  };
  readonly updates_status: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: UpdatesView;
  };
  readonly updates_whats_new: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: WhatsNew | null;
  };
  readonly voice_devices: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: readonly AudioDevice[];
  };
  readonly voice_dictation: {
    readonly args: { readonly action: DictationAction };
    readonly result: VoiceFeatures;
  };
  readonly voice_engines: {
    readonly args: { readonly action: EnginesAction };
    readonly result: VoiceFeatures;
  };
  readonly voice_features: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: VoiceFeatures;
  };
  readonly voice_preview: { readonly args: { readonly agent: string }; readonly result: null };
  readonly voice_ptt: { readonly args: { readonly pressed: boolean }; readonly result: null };
  readonly voice_read: {
    readonly args: { readonly action: ReadAction };
    readonly result: VoiceFeatures;
  };
  readonly voice_set_mic_enabled: {
    readonly args: { readonly enabled: boolean };
    readonly result: null;
  };
  readonly voice_set_muted: { readonly args: { readonly muted: boolean }; readonly result: null };
  readonly voice_speaker: {
    readonly args: { readonly action: SpeakerAction };
    readonly result: VoiceFeatures;
  };
  readonly voice_start_mic_test: {
    readonly args: { readonly deviceId: string | null };
    readonly result: null;
  };
  readonly voice_status: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: VoiceStatus;
  };
  readonly voice_stop_mic_test: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: null;
  };
  readonly voice_stop_speech: {
    readonly args: Readonly<Record<string, never>>;
    readonly result: null;
  };
  readonly voice_wake: {
    readonly args: { readonly action: WakeAction };
    readonly result: VoiceFeatures;
  };
}

/** Nazwa komendy rdzenia. */
export type IpcCommandName = keyof IpcCommands;

/** Ładunek kanału `alfa://events`. */
export type IpcEventFrame = EventFrame;
/** Odrzucenie komendy. */
export type IpcError = AppError;

/** Powód, dla którego polecenie wymaga lokalnej maszyny (B.5, `crates/app-api/data/local-only.json`). */
export type LocalReason = 'system_dialog' | 'asset_preview' | 'host_path' | 'host_desktop';
/**
 * Polecenia zakładające lokalną maszynę (okno systemu, `asset:`, ścieżka hosta, okna i schowek hosta):
 * klient zdalny ich nie dostanie — interfejs po kanale zdalnym chowa ich kontrolki albo używa odpowiednika.
 */
export const LOCAL_ONLY_COMMANDS = {
  accounts_import_env: 'host_desktop',
  agents_open_terminal: 'host_desktop',
  app_open_system_settings: 'host_desktop',
  attachments_add_dropped: 'host_desktop',
  attachments_paste: 'host_desktop',
  attachments_pick: 'system_dialog',
  backups_choose_dir: 'system_dialog',
  backups_restore: 'host_path',
  bridges_open_login: 'host_desktop',
  diag_export: 'host_path',
  files_act: 'system_dialog',
  files_preview: 'asset_preview',
  mcp_save: 'host_path',
  memory_teach_pick_file: 'system_dialog',
  permissions_open_approval: 'host_desktop',
  projects_pick_dir: 'system_dialog',
  projects_save: 'host_path',
  quick_expand_to_main: 'host_desktop',
  quick_hide: 'host_desktop',
  sessions_choose_workdir: 'system_dialog',
  sessions_export: 'system_dialog',
  sessions_export_conversation: 'system_dialog',
  sessions_workdir: 'host_path',
  skills_export: 'system_dialog',
  skills_import: 'system_dialog',
  system_clipboard_text: 'host_desktop',
  transfer_export: 'system_dialog',
  transfer_import: 'system_dialog',
  transfer_inspect: 'system_dialog',
  turns_save_code: 'system_dialog',
  updates_restart: 'host_desktop',
} as const satisfies Partial<Record<IpcCommandName, LocalReason>>;
