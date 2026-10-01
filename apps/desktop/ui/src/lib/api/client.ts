// Interfejs warstwy danych UI. Dwie implementacje: `TauriAlfaClient` (IPC; komendy i zdarzenia
// opisane w COMMANDS.md) i `FakeAlfaClient` (w pamięci, deterministyczny — Storybook, testy, dev).
// Metody oznaczone „intencja" nie wykonują akcji w UI: przekazują życzenie do rdzenia
// (np. zatwierdzenie → okno Brokera, zapis pliku → natywny dialog, czytanie na głos → TTS).
import type { AgentId } from '@alfa/ui-kit';
import type {
  AgentState,
  ArtifactAction,
  ArtifactInfo,
  ArtifactPreview,
  AutonomyLevel,
  CastTemplateId,
  CostSummary,
  Money,
  RememberScope,
  SendOptions,
  SendResult,
  SessionSearchHit,
  SessionSummary,
  SessionTemplate,
  TimelineEvent,
  TimelineFilter,
  TurnsSnapshot,
  UndoTicket,
} from './types';
import type { AgentRunDetail, SessionWorkdir, VoiceStatus, WorkdirChoice } from './types-agents';
import type {
  Account,
  AccountAssignment,
  AddAccountInput,
  AudioDevice,
  BrokerIntentResult,
  DeviceProfile,
  ExportRequest,
  ExportResult,
  ImportRequest,
  ImportResult,
  InspectResult,
  LocalModelInfo,
  PermissionsState,
  ProviderInfo,
  TestReport,
} from './types-hub';
import type {
  AlfaEvent,
  AppBootstrap,
  LayoutPrefs,
  QuickAskResult,
  SettingValue,
  SettingsPageDef,
  SystemStatus,
  Unsubscribe,
} from './types-system';

export interface AppApi {
  bootstrap(): Promise<AppBootstrap>;
  completeOnboarding(): Promise<void>;
  /** Intencja: otwiera stronę ustawień Windows (tylko `ms-settings:` z listy dozwolonych w rdzeniu). */
  openSystemSettings(
    uri: 'ms-settings:privacy-microphone' | 'ms-settings:storagesense',
  ): Promise<void>;
  saveLayout(layout: LayoutPrefs): Promise<void>;
  setActiveSession(sessionId: string | null): Promise<void>;
}

export interface SessionsApi {
  list(): Promise<readonly SessionSummary[]>;
  create(template: SessionTemplate): Promise<SessionSummary>;
  rename(sessionId: string, title: string): Promise<void>;
  setPinned(sessionId: string, pinned: boolean): Promise<void>;
  setArchived(sessionId: string, archived: boolean): Promise<void>;
  /** Usunięcie cofalne przez 10 s. */
  remove(sessionId: string): Promise<UndoTicket>;
  undoRemove(token: string): Promise<void>;
  /** Intencja: duplikuj jako szablon. */
  duplicateAsTemplate(sessionId: string): Promise<SessionSummary>;
  /** Intencja: eksport pojedynczej sesji do `.alfa` (natywny dialog zapisu). */
  exportSession(sessionId: string): Promise<ExportResult>;
  search(query: string): Promise<readonly SessionSearchHit[]>;
  markRead(sessionId: string): Promise<void>;
  getDraft(sessionId: string): Promise<string>;
  saveDraft(sessionId: string, text: string): Promise<void>;
  /** Katalog roboczy sesji = zakres narzędzi agentek (`path: null` — agentki bez narzędzi). */
  workdir(sessionId: string): Promise<SessionWorkdir>;
  /** Intencja: wybór katalogu (natywny dialog), katalog sesji albo wyłączenie narzędzi. */
  chooseWorkdir(sessionId: string, choice: WorkdirChoice): Promise<SessionWorkdir>;
}

export interface TurnsApi {
  /** Całe drzewo gałęzi sesji (append-only) + adnotacje widoku. */
  list(sessionId: string): Promise<TurnsSnapshot>;
  send(sessionId: string, options: SendOptions): Promise<SendResult>;
  /** Ponów → nowy wariant obok (rodzeństwo tury agentki). */
  regenerate(sessionId: string, turnId: string, profile: string | null): Promise<string>;
  /** Edytuj i wyślij ponownie → nowa gałąź od tego miejsca (rodzeństwo tury użytkownika). */
  editAndResend(sessionId: string, turnId: string, text: string): Promise<SendResult>;
  /** Kontynuuj uciętą odpowiedź → nowa tura-dziecko. */
  continueTurn(sessionId: string, turnId: string): Promise<string>;
  stop(sessionId: string): Promise<void>;
  rate(turnId: string, rating: 'up' | 'down' | null): Promise<void>;
  setHidden(turnId: string, hidden: boolean): Promise<void>;
  /** Intencja: zapisz do pamięci. */
  remember(turnId: string, scope: RememberScope): Promise<void>;
  /** Intencja: przeczytaj na głos głosem tej agentki. */
  readAloud(turnId: string): Promise<void>;
  /** Intencja: zapisz blok kodu jako plik (natywny dialog). */
  saveCode(turnId: string, blockIndex: number): Promise<void>;
  /** Intencja: uruchom blok kodu w terminalu — zawsze przez Broker. */
  runCode(turnId: string, blockIndex: number): Promise<BrokerIntentResult>;
  /** Intencja: cofnij krok narzędzia (dziennik cofania). */
  undoStep(undoToken: string): Promise<void>;
}

export interface AgentsApi {
  list(sessionId: string): Promise<readonly AgentState[]>;
  /** Zmiana obsady jest natychmiastowa i trafia do dziennika (PLAN §9.2). */
  setRoles(sessionId: string, agent: AgentId, roleIds: readonly string[]): Promise<void>;
  applyCast(sessionId: string, template: CastTemplateId): Promise<void>;
  /** Przebiegi agentek z krokami (Replay na Osi czasu). */
  runs(sessionId: string): Promise<readonly AgentRunDetail[]>;
  /** Wiadomość w trakcie zadania — agentka uwzględnia ją w następnym kroku (PLAN §9.6). */
  steer(sessionId: string, text: string): Promise<void>;
  /** Intencja: terminal w katalogu kroku „uruchom w terminalu" (polecenie NIE jest wykonywane). */
  openTerminal(stepId: string): Promise<void>;
}

export interface CostsApi {
  summary(sessionId: string | null): Promise<CostSummary>;
  /** Limit miesięczny w PLN — z możliwością całkowitego wyłączenia (PLAN §14.6). */
  setMonthlyLimit(enabled: boolean, monthly: Money): Promise<void>;
}

export interface SettingsApi {
  schema(): Promise<readonly SettingsPageDef[]>;
  values(): Promise<Readonly<Record<string, SettingValue>>>;
  set(key: string, value: SettingValue): Promise<void>;
  reset(key: string): Promise<SettingValue>;
  setShortcut(actionId: string, chord: string | null): Promise<void>;
}

export interface TimelineApi {
  list(sessionId: string, filter: TimelineFilter): Promise<readonly TimelineEvent[]>;
}

export interface FilesApi {
  list(sessionId: string): Promise<readonly ArtifactInfo[]>;
  preview(artifactId: string): Promise<ArtifactPreview>;
  /** Intencja: Otwórz · Pokaż w Eksploratorze · Kopiuj · Zapisz jako… */
  act(artifactId: string, action: ArtifactAction): Promise<void>;
}

export interface AccountsApi {
  catalog(): Promise<readonly ProviderInfo[]>;
  list(): Promise<readonly Account[]>;
  add(input: AddAccountInput): Promise<Account>;
  test(accountId: string): Promise<TestReport>;
  assign(accountId: string, assignment: AccountAssignment): Promise<void>;
  setLimit(accountId: string, enabled: boolean, monthly: Money): Promise<void>;
  remove(accountId: string): Promise<void>;
}

export interface TransferApi {
  /** Intencja: natywny dialog zapisu + eksport. */
  exportPackage(request: ExportRequest): Promise<ExportResult>;
  /** Intencja: osobna, jawna paczka sekretów — zawsze szyfrowana hasłem (min. 8 znaków). */
  exportSecrets(password: string): Promise<ExportResult>;
  /** Intencja: natywny dialog otwarcia + podgląd (dry-run). */
  inspect(password: string | null, path: string | null): Promise<InspectResult>;
  importPackage(request: ImportRequest): Promise<ImportResult>;
  rollback(snapshotId: string): Promise<void>;
}

export interface PermissionsApi {
  get(sessionId: string | null): Promise<PermissionsState>;
  /** Intencja: zmiana poziomu autonomii — potwierdzenie wyłącznie w oknie Brokera. */
  requestLevel(level: AutonomyLevel, sessionId: string | null): Promise<BrokerIntentResult>;
  /** Intencja: przenosi do okna Brokera z tą prośbą o zatwierdzenie. */
  openApproval(approvalId: string): Promise<BrokerIntentResult>;
}

export interface ModelsApi {
  /** Modele lokalne z manifestu (`providers-local`). */
  localList(): Promise<readonly LocalModelInfo[]>;
  /** Pobieranie (wznawiane, SHA-256); `null` = model domyślny. Postęp: `LocalModelProgress`. */
  localDownload(modelId: string | null): Promise<void>;
  /** Anulowanie pobierania (`null` = wszystkich). */
  localCancel(modelId: string | null): Promise<void>;
}

export interface DeviceApi {
  profile(): Promise<DeviceProfile>;
  measure(): Promise<DeviceProfile>;
}

export interface VoiceApi {
  devices(): Promise<readonly AudioDevice[]>;
  /** Test mikrofonu: poziomy przychodzą zdarzeniem `MicLevel` (≤ 30/s). */
  startMicTest(deviceId: string | null): Promise<void>;
  stopMicTest(): Promise<void>;
  setMicEnabled(enabled: boolean): Promise<void>;
  setMuted(muted: boolean): Promise<void>;
  stopSpeech(): Promise<void>;
  /** Stan trybu głosowego (bez modeli — `unavailable` z powodem). */
  status(): Promise<VoiceStatus>;
  /** Mówienie z przytrzymaniem (Spacja / przycisk mikrofonu). */
  ptt(pressed: boolean): Promise<void>;
  /** Intencja: próbka głosu agentki (głosy v0). */
  preview(agent: AgentId): Promise<void>;
}

export interface SystemApi {
  status(): Promise<SystemStatus>;
  /** Ponów wysłanie kolejki offline. */
  retryQueue(): Promise<void>;
}

export interface QuickApi {
  ask(text: string): Promise<QuickAskResult>;
  /** `Enter` → otwórz w pełnym oknie z tą samą sesją. */
  expandToMain(sessionId: string): Promise<void>;
  hide(): Promise<void>;
}

export interface AlfaClient {
  readonly kind: 'tauri' | 'fake';
  readonly app: AppApi;
  readonly sessions: SessionsApi;
  readonly turns: TurnsApi;
  readonly agents: AgentsApi;
  readonly costs: CostsApi;
  readonly settings: SettingsApi;
  readonly timeline: TimelineApi;
  readonly files: FilesApi;
  readonly accounts: AccountsApi;
  readonly transfer: TransferApi;
  readonly permissions: PermissionsApi;
  readonly models: ModelsApi;
  readonly device: DeviceApi;
  readonly voice: VoiceApi;
  readonly system: SystemApi;
  readonly quick: QuickApi;
  /** Jeden kanał zdarzeń; rdzeń wysyła je paczkami (batch co klatkę). */
  subscribe(handler: (batch: readonly AlfaEvent[]) => void): Unsubscribe;
  dispose(): void;
}
