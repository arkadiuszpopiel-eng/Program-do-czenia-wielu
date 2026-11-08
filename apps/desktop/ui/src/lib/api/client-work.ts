// Interfejsy warstwy danych: panel „Ekran" (computer use), wbudowany terminal, umiejętności,
// Kreator agentek i „Zdrowie systemu" (część `AlfaClient`; komendy w COMMANDS.md).
import type { BrokerIntentResult, ExportResult } from './types-hub';
import type { TaskInfo } from './types-tasks';
import type {
  AgentDraft,
  BuilderAgentInfo,
  BuilderDryRun,
  BuilderPolicyView,
  BuilderPreview,
  BuilderProposal,
  BuilderSaved,
  EvalSuiteView,
  EvalsView,
  GuiScreenshot,
  GuiStatus,
  HealthView,
  ImproverView,
  SkillImportResult,
  SkillInfo,
  SkillReview,
  TerminalFrame,
  TerminalProfileId,
  TerminalSession,
} from './types-work';

/** Panel „Ekran": co agentka widzi i robi; przejęcie sterowania. */
export interface GuiApi {
  status(): Promise<GuiStatus>;
  /** Ostatni zamaskowany zrzut agentki (tylko w pamięci rdzenia). */
  screenshot(): Promise<GuiScreenshot | null>;
  /** „Zatrzymaj sterowanie": anulowanie + przejęcie (narzędzia GUI wstrzymane). */
  stop(): Promise<GuiStatus>;
  /** „Oddaj sterowanie". */
  release(): Promise<GuiStatus>;
  /** Intencja: „zawsze zezwalaj na podgląd pulpitu" — decyzja wyłącznie w oknie Brokera. */
  desktopGrant(sessionId: string, agent: string): Promise<BrokerIntentResult>;
}

/** Wbudowany terminal — sterowany wyłącznie przez użytkownika (gest w panelu). */
export interface TerminalApi {
  /** Strumień VT przychodzi tylko do `onFrame` (kanał IPC), nigdy przez `alfa://events`. */
  open(
    profile: TerminalProfileId,
    cols: number,
    rows: number,
    cwd: string | null,
    onFrame: (frame: TerminalFrame) => void,
  ): Promise<TerminalSession>;
  /** Klawiatura panelu (base64, ≤ 64 KiB). */
  input(terminal: number, dataB64: string): Promise<void>;
  resize(terminal: number, cols: number, rows: number): Promise<void>;
  /** Zamyka i zabija drzewo procesów. */
  close(terminal: number): Promise<void>;
  list(): Promise<readonly TerminalSession[]>;
}

/** Umiejętności: biblioteka, przegląd propozycji (diff + hash), kwarantanna. */
export interface SkillsApi {
  list(): Promise<readonly SkillInfo[]>;
  review(skillId: string, version: string): Promise<SkillReview>;
  /** Przepis właściciela (JSON) → propozycja (walidacja, testy akceptacyjne, skaner). */
  propose(skill: unknown): Promise<SkillInfo>;
  /** Instalacja — tylko kliknięcie w UI, hash przejrzanej wersji. */
  approve(skillId: string, version: string, hash: string): Promise<SkillInfo>;
  /** Zwolnienie z kwarantanny — tylko kliknięcie w UI. */
  release(skillId: string, version: string, hash: string): Promise<SkillInfo>;
  reject(skillId: string, version: string): Promise<SkillInfo>;
  disable(skillId: string): Promise<SkillInfo>;
  /** Uruchomienie jako zadanie agentki w sesji (koperta ≤ jej roli). */
  run(skillId: string, sessionId: string, agent: string | null, params: unknown): Promise<TaskInfo>;
  /** Intencja: natywne „Zapisz jako". */
  exportBundle(): Promise<ExportResult>;
  /** Intencja: natywne „Otwórz" — import zawsze jako propozycje (z zewnątrz → kwarantanna). */
  importBundle(): Promise<SkillImportResult>;
}

/** Kreator agentek: szkic → podgląd → test na sucho → zapis (nie głosem). */
export interface BuilderApi {
  policy(): Promise<BuilderPolicyView>;
  propose(description: string): Promise<BuilderProposal>;
  preview(draft: AgentDraft): Promise<BuilderPreview>;
  dryRun(draft: AgentDraft): Promise<BuilderDryRun>;
  /** Zapis po zaliczonym teście na sucho tego samego `hash`. */
  save(draft: AgentDraft, hash: string): Promise<BuilderSaved>;
  /** Intencja: odsłuch głosu v0 (mówczyni bazowa). */
  voicePreview(draft: AgentDraft): Promise<void>;
  library(): Promise<readonly BuilderAgentInfo[]>;
}

/** „Zdrowie systemu": Diagnosta, Ulepszacz, evale. */
export interface HealthApi {
  report(): Promise<HealthView>;
  scan(): Promise<HealthView>;
  /** Zgoda na naprawę (Jądro — wyłącznie okno Brokera). */
  approve(repairId: number): Promise<HealthView>;
  reject(repairId: number): Promise<HealthView>;
  /** „Cofnij" naprawę. */
  undo(repairId: number): Promise<HealthView>;
  improver(): Promise<ImproverView>;
  /** „Przeanalizuj teraz" (ręcznie). */
  improverCycle(): Promise<ImproverView>;
  /** Zatwierdzenie dokładnie tego diffu (`digest`). */
  improverApprove(proposalId: number, digest: string): Promise<ImproverView>;
  improverReject(proposalId: number): Promise<ImproverView>;
  improverRollback(proposalId: number): Promise<ImproverView>;
  evals(): Promise<EvalsView>;
  evalsVerify(suiteId: string): Promise<EvalSuiteView>;
}
