// Interfejsy kont i kluczy API oraz importu/eksportu `.alfa` (część `AlfaClient`, COMMANDS.md).
// Wydzielone z `client.ts` (limit 400 linii na plik).
import type { Money } from './types';
import type {
  Account,
  AccountAssignment,
  AddAccountInput,
  EnterpriseSettings,
  EnvImportResult,
  ExportRequest,
  ExportResult,
  ImportRequest,
  ImportResult,
  InspectResult,
  ModelRouting,
  ModelRoutingSettings,
  ProviderInfo,
  RoutingPreview,
  SharedQueueEntry,
  SharedQueues,
  TestReport,
} from './types-hub';

export interface AccountsApi {
  catalog(): Promise<readonly ProviderInfo[]>;
  list(): Promise<readonly Account[]>;
  add(input: AddAccountInput): Promise<Account>;
  test(accountId: string): Promise<TestReport>;
  assign(accountId: string, assignment: AccountAssignment): Promise<void>;
  setLimit(accountId: string, enabled: boolean, monthly: Money): Promise<void>;
  remove(accountId: string): Promise<void>;
  /**
   * Wymiana klucza istniejącego konta. Nowy klucz jest najpierw sprawdzany; przy `ok: false`
   * konto i stary klucz zostają bez zmian.
   */
  rotate(accountId: string, secret: string): Promise<TestReport>;
  /** Wyłączone konto zostaje na liście z kluczem, ale router go nie używa. */
  setDisabled(accountId: string, disabled: boolean): Promise<Account>;
  /**
   * Ustawienia dla firm istniejącego konta: organizacja i projekt OpenAI albo wersja API
   * i wdrożenia Azure OpenAI (adres zasobu zostaje). Ten sam klucz w tym samym projekcie OpenAI
   * co inne konto — odmowa.
   */
  setEnterprise(accountId: string, enterprise: EnterpriseSettings): Promise<Account>;
  /** Model wpisany ręcznie — dla klucza, który nie ma prawa do listy modeli. */
  setModel(accountId: string, model: string): Promise<Account>;
  /**
   * Kolejka modeli konta dla klasy zadań (kolejność prób Routera, najwyżej 10). Pusta lista
   * przywraca wybór automatyczny; model niepasujący do klasy jest odrzucany z wyjaśnieniem.
   */
  setQueue(accountId: string, taskClass: string, models: readonly string[]): Promise<Account>;
  /** Propozycja kolejki (`quality` / `balanced` / `economy`) — niczego nie zapisuje. */
  suggestQueue(accountId: string, taskClass: string, strategy: string): Promise<string[]>;
  /** Wspólna kolejka wszystkich kont (pierwszeństwo przed kolejkami kont). */
  sharedQueues(): Promise<SharedQueues>;
  /** Wspólna kolejka klasy zadań (najwyżej 10 pozycji); pusta lista ją usuwa. */
  setSharedQueue(taskClass: string, entries: readonly SharedQueueEntry[]): Promise<SharedQueues>;
  /** Dobór modelu według trudności: ustawienia i modele Routera z ich szczeblami. */
  routing(): Promise<ModelRouting>;
  /** Zapisuje ustawienia doboru (działają od następnej odpowiedzi). */
  setRouting(settings: ModelRoutingSettings): Promise<ModelRouting>;
  /** Jak Router potraktuje przykładowe zadanie w klasie (`chat`, `code`, …). */
  routingPreview(text: string, taskClass: string): Promise<RoutingPreview>;
  /** Import kluczy ze zmiennych środowiskowych z katalogu dostawców (raport bez wartości). */
  importEnv(): Promise<readonly EnvImportResult[]>;
}

export interface TransferApi {
  /** Intencja: natywny dialog zapisu + eksport. */
  exportPackage(request: ExportRequest): Promise<ExportResult>;
  /**
   * Podgląd (dry-run). `handle: null` — intencja: natywny dialog otwarcia; inaczej jednorazowy
   * uchwyt z poprzedniego `InspectResult` albo z `backups.restore` (ścieżek UI nie podaje).
   */
  inspect(password: string | null, handle: string | null): Promise<InspectResult>;
  importPackage(request: ImportRequest): Promise<ImportResult>;
  rollback(snapshotId: string): Promise<void>;
}
