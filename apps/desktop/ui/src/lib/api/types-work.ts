// DTO: panel „Ekran" (computer use), wbudowany terminal, umiejętności, Kreator agentek i strona
// „Zdrowie systemu". Odpowiedniki `crates/app-api/src/dto/{gui,skills,health}.rs` (pola snake_case).
import type { AutonomyLevel, Iso8601, LocalizedText } from './types';

// ── Ekran (computer use) ────────────────────────────────────────────────────────────────────────

export type GuiActionStatus = 'running' | 'ok' | 'denied' | 'failed' | 'cancelled';

/** Kto steruje teraz (wskaźnik w pasku tytułu). */
export interface GuiControl {
  readonly session_id: string;
  readonly agent: string;
  readonly tool: string;
  readonly since: Iso8601;
}

/** Akcja GUI agentki — bez treści wpisywanej (tylko rodzaj, cel, liczba znaków). */
export interface GuiAction {
  readonly id: number;
  readonly at: Iso8601;
  readonly session_id: string;
  readonly agent: string;
  readonly tool: string;
  readonly title: string;
  readonly target: string | null;
  readonly status: GuiActionStatus;
  readonly summary: string;
  readonly duration_ms: number | null;
}

/** Metadane ostatniego zrzutu (piksele tylko przez `gui_screenshot`, nigdy w zdarzeniach). */
export interface GuiShotInfo {
  readonly at: Iso8601;
  readonly session_id: string;
  readonly agent: string;
  readonly width: number;
  readonly height: number;
  readonly masked: number;
  readonly black_frame: boolean;
}

export interface GuiStatus {
  readonly available: boolean;
  readonly reason: LocalizedText | null;
  readonly control: GuiControl | null;
  /** Właściciel przejął sterowanie — narzędzia GUI wstrzymane do „Oddaj sterowanie". */
  readonly taken_over: boolean;
  /** Ostatnie akcje (najnowsze pierwsze). */
  readonly actions: readonly GuiAction[];
  readonly screenshot: GuiShotInfo | null;
}

/** Ostatni zrzut agentki, zamaskowany w porcie (okna Alfy/Brokera, deny-lista, pola haseł). */
export interface GuiScreenshot {
  readonly info: GuiShotInfo;
  readonly data_url: string;
}

// ── Terminal ────────────────────────────────────────────────────────────────────────────────────

export type TerminalProfileId = 'shell' | 'cmd' | 'claude_login' | 'codex_login';

export interface TerminalSession {
  readonly id: number;
  readonly profile: TerminalProfileId;
  readonly pid: number;
  readonly alive: boolean;
}

/** Ramka strumienia — wyłącznie kanał `Channel` otwarcia (nigdy `alfa://events`). */
export type TerminalFrame =
  | { readonly kind: 'output'; readonly data_b64: string }
  | { readonly kind: 'exit'; readonly code: number | null };

// ── Umiejętności ────────────────────────────────────────────────────────────────────────────────

export type SkillStateView =
  'proposed' | 'quarantined' | 'installed' | 'rejected' | 'disabled' | 'superseded';

export type SkillOrigin = 'memory' | 'user' | 'own_package' | 'external';

export interface SkillInfo {
  readonly id: string;
  readonly version: string;
  readonly name: string;
  readonly description: string;
  readonly state: SkillStateView;
  readonly origin: SkillOrigin;
  readonly trusted: boolean;
  /** SHA-256 treści — potwierdzany przy instalacji (przejrzana wersja = instalowana). */
  readonly hash: string;
  /** Uwagi skanera (powody kwarantanny). */
  readonly findings: readonly string[];
  readonly keywords: readonly string[];
  readonly required_tools: readonly string[];
  readonly required_capabilities: readonly string[];
  /** JSON Schema parametrów (obiekt). */
  readonly parameters: unknown;
  readonly proposed_at: Iso8601;
  readonly decided_at: Iso8601 | null;
}

export type DiffKind = 'same' | 'added' | 'removed';

export interface DiffLine {
  readonly kind: DiffKind;
  readonly text: string;
}

export interface SkillReview {
  readonly skill: SkillInfo;
  readonly previous_version: string | null;
  readonly diff: readonly DiffLine[];
}

export interface SkillImportResult {
  readonly proposed: readonly SkillInfo[];
  readonly skipped: readonly string[];
}

// ── Kreator agentek ─────────────────────────────────────────────────────────────────────────────

export interface NameForms {
  readonly nominative: string;
  readonly genitive: string;
  readonly dative: string;
  readonly accusative: string;
  readonly instrumental: string;
  readonly locative: string;
  readonly vocative: string;
}

export interface VoiceDraft {
  readonly base: string;
  readonly pitch: number;
  readonly rate: number;
  readonly perceived_age: number;
  readonly timbre: string;
  readonly design_prompt: string;
}

export interface RoleDraft {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly prompt: string;
  readonly model_policy: string;
  /** Grupy narzędzi (⊆ dozwolone przez politykę Kreatora). */
  readonly tools: readonly string[];
  readonly read_only: boolean;
  readonly untrusted_isolated: boolean;
  readonly author: boolean;
}

export interface RunBudgetDraft {
  readonly max_steps: number;
  readonly max_tokens: number;
  readonly max_wall_ms: number;
  readonly max_cost_micro_usd: number | null;
  readonly max_tool_calls_per_turn: number;
}

export interface LimitsDraft {
  readonly autonomy: AutonomyLevel | null;
  readonly budget: RunBudgetDraft | null;
  readonly fs_write: readonly string[];
  readonly memory_scope: string | null;
  readonly retain_days: number | null;
  readonly triggers: readonly string[];
}

/** Szkic agentki (formularz albo wynik rozmowy) — kształt `agent_builder_contract::AgentDraft`. */
export interface AgentDraft {
  readonly id: string | null;
  readonly name: string | null;
  readonly forms: NameForms | null;
  readonly glyph: string | null;
  readonly color: string | null;
  readonly character: string | null;
  readonly voice: VoiceDraft | null;
  readonly role: RoleDraft | null;
  readonly limits: LimitsDraft;
  readonly skills: readonly string[];
}

export interface BuilderPolicyView {
  readonly groups: readonly string[];
  /** Sufit autonomii nowej agentki (poziom z Brokera, nigdy L4). */
  readonly ceiling: AutonomyLevel;
  readonly palette: readonly string[];
  readonly voices: readonly string[];
  readonly model_policies: readonly string[];
  readonly max_steps: number;
}

export interface BuilderProposal {
  readonly draft: AgentDraft;
  readonly questions: readonly string[];
}

export interface BuilderPreview {
  readonly hash: string;
  readonly persona_id: string;
  readonly name: string;
  /** Odmiana: mianownik … wołacz. */
  readonly forms: readonly string[];
  readonly glyph: string;
  readonly color: string;
  readonly character: string;
  readonly role_id: string;
  readonly role_name: string;
  readonly groups: readonly string[];
  readonly read_only: boolean;
  readonly tools: readonly string[];
  readonly voice: string;
  readonly autonomy: AutonomyLevel;
  readonly system_prompt: string;
  readonly fs_write: readonly string[];
  readonly memory_scope: string;
  readonly retain_days: number;
  readonly max_steps: number;
  readonly warnings: readonly string[];
}

export type DryOutcome = 'allowed' | 'ask' | 'denied';

export interface BuilderDryStep {
  readonly tool: string;
  readonly expected: DryOutcome;
  readonly outcome: DryOutcome;
  readonly why: string;
}

export interface BuilderDryRun {
  readonly hash: string;
  readonly passed: boolean;
  readonly steps: readonly BuilderDryStep[];
}

export interface BuilderSaved {
  readonly persona: string;
  readonly role: string;
  readonly hash: string;
}

export interface BuilderAgentInfo {
  readonly persona: string;
  readonly name: string;
  readonly color: string;
  readonly role: string;
  readonly autonomy: AutonomyLevel;
  readonly hash: string;
}

// ── Zdrowie systemu ─────────────────────────────────────────────────────────────────────────────

export type HealthOverall = 'ok' | 'degraded' | 'failing' | 'safe_mode';
export type ModuleHealth = 'healthy' | 'degraded' | 'unhealthy' | 'not_started' | 'disabled';
export type RiskView = 'low' | 'medium' | 'high';

export interface HealthModule {
  readonly module: string;
  readonly version: string;
  readonly lifecycle: string;
  readonly health: ModuleHealth;
  readonly detail: string | null;
}

export interface HealthIncident {
  readonly id: number;
  readonly kind: string;
  readonly title: string;
  readonly target: string;
  readonly count: number;
  readonly last_at: Iso8601;
  readonly status: string;
}

export interface HealthRepair {
  readonly id: number;
  readonly title: string;
  readonly diff: readonly string[];
  readonly at: Iso8601;
  readonly undoable: boolean;
}

export interface HealthHumanAction {
  readonly id: number;
  readonly title: string;
  readonly what: string;
  readonly mitigated: boolean;
}

/** Propozycja naprawy (Jądro — zatwierdza wyłącznie okno Brokera). */
export interface HealthProposal {
  readonly id: number;
  readonly title: string;
  readonly diff: readonly string[];
  readonly rationale: string;
  readonly risk: RiskView;
  readonly rollback_plan: readonly string[];
  readonly kernel: boolean;
}

export interface HealthView {
  readonly overall: HealthOverall;
  readonly safe_mode: string | null;
  readonly generated_at: Iso8601;
  readonly modules: readonly HealthModule[];
  readonly incidents: readonly HealthIncident[];
  readonly repaired: readonly HealthRepair[];
  readonly needs_human: readonly HealthHumanAction[];
  readonly pending: readonly HealthProposal[];
  readonly problems: readonly string[];
}

export interface ImproverChange {
  readonly key: string;
  readonly old: unknown;
  readonly new: unknown;
  readonly ring: string;
  readonly safety: string;
}

export interface ImproverProposalView {
  readonly id: number;
  readonly title: string;
  readonly rationale: string;
  readonly source: string;
  readonly ring: string;
  readonly safety: string;
  readonly stage: string;
  readonly note: string | null;
  /** Hash diffu — zatwierdzasz dokładnie ten diff. */
  readonly digest: string;
  readonly changes: readonly ImproverChange[];
  readonly created_at: Iso8601;
  readonly needs_signature: boolean;
  readonly can_approve: boolean;
  readonly can_rollback: boolean;
}

export interface ImproverBlocked {
  readonly at: Iso8601;
  readonly source: string;
  readonly target: string;
  readonly violation: string;
}

export interface ImproverIssue {
  readonly at: Iso8601;
  readonly title: string;
  readonly path: string;
  readonly body: string;
}

export interface ImproverView {
  readonly proposals: readonly ImproverProposalView[];
  readonly blocked: readonly ImproverBlocked[];
  readonly issues: readonly ImproverIssue[];
  /** Cykl w bezczynności aktywny (jest port bezczynności); inaczej tylko ręcznie. */
  readonly idle_cycle: boolean;
  readonly last_cycle: Iso8601 | null;
}

export interface EvalSuiteView {
  readonly id: string;
  readonly wave: string;
  readonly version: string;
  readonly status: string;
  readonly integrity_ok: boolean;
  readonly problems: readonly string[];
  readonly thresholds: number;
}

/** Werdykt bramki — wynik zbiorczy (holdout bez danych przypadków). */
export interface EvalVerdictView {
  readonly at: Iso8601;
  readonly suite: string;
  readonly stage: string;
  readonly passed: boolean;
  readonly summary: string;
}

export interface EvalsView {
  readonly available: boolean;
  readonly reason: string | null;
  readonly suites: readonly EvalSuiteView[];
  readonly holdout_suites: number;
  readonly verdicts: readonly EvalVerdictView[];
}
