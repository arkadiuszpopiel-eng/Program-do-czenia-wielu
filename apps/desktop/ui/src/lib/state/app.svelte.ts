// Stan aplikacji okna głównego: start, zdarzenia (paczka raz na klatkę), sesje, rozmowa,
// ustawienia, stan systemu, koszty, agentki. Komponenty czytają pola, akcje wołają metody.
import type { MicState } from '@alfa/ui-kit';
import type { AlfaClient } from '../api/client';
import type {
  ActivityInfo,
  AgentState,
  CostSummary,
  SessionSummary,
  SessionTemplate,
} from '../api/types';
import type {
  AlfaEvent,
  AppBootstrap,
  ChatStreamEvent,
  PanelId,
  SettingValue,
  SystemStatus,
} from '../api/types-system';

/** Zdarzenie postępu pobierania modelu lokalnego. */
export type LocalModelProgress = Extract<AlfaEvent, { type: 'LocalModelProgress' }>;
import { errorText } from '../api/command-error';
import { i18n, type I18n } from '../i18n/i18n.svelte';
import { RafBatcher, type FrameScheduler } from '../logic/raf-batcher';
import { SentenceAnnouncer, type Timers } from '../logic/sentence-announcer';
import { SHORTCUTS } from '../logic/shortcut-registry';
import { buildKeymap, effectiveBindings } from '../logic/shortcuts';
import { applyEvent } from './apply-batch';
import { attempt, showError } from './attempt';
import { Debouncer, DraftSaver, logFailure } from './background';
import { ConversationState } from './conversation.svelte';
import { LayoutState } from './layout.svelte';
import { RunsState } from './runs.svelte';
import * as sessionActions from './session-actions';
import { SessionsState } from './sessions.svelte';
import * as settingsActions from './settings-actions';
import { ToastState } from './toasts.svelte';
import { VoiceUiState } from './voice.svelte';
import { UpdatesState } from './updates.svelte';
import { BrokerState } from './broker.svelte';
import { WorkState } from './work.svelte';

export type View = 'loading' | 'chat' | 'settings' | 'onboarding' | 'error';

export interface AppOptions {
  readonly frames?: FrameScheduler;
  readonly timers?: Timers;
  /** Opóźnienie zapisu szkicu/układu (ms). */
  readonly debounceMs?: number;
}

const CACHE_SIZE = 6;

export class AppState {
  readonly i18n: I18n = i18n;
  readonly toasts = new ToastState();
  readonly sessions = new SessionsState();
  readonly runs = new RunsState();
  readonly voice = new VoiceUiState();
  readonly work = new WorkState();
  readonly updates = new UpdatesState();
  readonly broker = new BrokerState();
  readonly layout: LayoutState;

  view = $state<View>('loading');
  fatal = $state<string | null>(null);
  settings = $state<Record<string, SettingValue>>({});
  shortcutOverrides = $state<Record<string, string>>({});
  system = $state<SystemStatus | null>(null);
  costs = $state<CostSummary | null>(null);
  agents = $state<Record<string, AgentState[]>>({});
  activity = $state<Record<string, ActivityInfo | null>>({});
  micState = $state<MicState>('off');
  micLevel = $state(0);
  conversation = $state.raw<ConversationState | null>(null);
  palette = $state<{ open: boolean; mode: 'all' | 'sessions' }>({ open: false, mode: 'all' });
  cheatsheetOpen = $state(false);
  findOpen = $state(false);
  settingsPage = $state('general');
  timelineTurn = $state<string | null>(null);
  /** Tura użytkownika edytowana w miejscu (edytuj → nowa gałąź). */
  editingTurnId = $state<string | null>(null);
  /** „Dodaj klucz" z banera / pustego stanu otwiera od razu kreator w Hubie kont. */
  hubWizard = $state(false);
  /** Krok startowy wprowadzenia (np. powrót do kroku albo makieta w Storybooku). */
  onboardingStep = $state(0);
  announcement = $state('');
  dismissed = $state<Record<string, boolean>>({});
  appVersion = $state('');
  /** Ostatni postęp pobierania modelu lokalnego (onboarding, Ustawienia). */
  localDownload = $state<LocalModelProgress | null>(null);

  readonly keymap = $derived(buildKeymap(SHORTCUTS, this.shortcutOverrides));
  readonly bindings = $derived(effectiveBindings(SHORTCUTS, this.shortcutOverrides));

  private readonly batcher: RafBatcher<AlfaEvent>;
  private readonly announcer: SentenceAnnouncer;
  private readonly cache: Record<string, ConversationState> = {};
  private cacheOrder: string[] = [];
  private unsubscribe: (() => void) | null = null;
  private listeners: ((event: AlfaEvent) => void)[] = [];
  private readonly debouncer: Debouncer;
  private readonly drafts: DraftSaver;

  constructor(
    readonly client: AlfaClient,
    options: AppOptions = {},
  ) {
    this.debouncer = new Debouncer(options.debounceMs ?? 400);
    // Układ zapisze się przy następnej zmianie — błąd bez toastu, ale nie po cichu.
    this.layout = new LayoutState((prefs) =>
      this.debouncer.run('layout', () => {
        this.client.app.saveLayout(prefs).catch(logFailure('saveLayout'));
      }),
    );
    this.drafts = new DraftSaver(
      (id, text) => this.client.sessions.saveDraft(id, text),
      (error) =>
        this.toasts.show({
          kind: 'error',
          message: this.i18n.t('composer.draftFailed', { error: errorText(error) }),
        }),
    );
    this.batcher = new RafBatcher((batch) => this.applyBatch(batch), { frames: options.frames });
    this.announcer = new SentenceAnnouncer((text) => (this.announcement = text), {
      timers: options.timers,
      codeLabel: i18n.t('msg.code', { lang: '' }).trim() + '.',
    });
  }

  get activeId(): string | null {
    return this.sessions.activeId;
  }

  /**
   * Start okna. Krytyczne (widok błędu z „Ponów"): ustawienia startowe, słownik, lista sesji.
   * Reszta (stan systemu, głos, koszty, agentki, szkic) — toast albo dziennik, okno działa dalej.
   */
  async start(): Promise<void> {
    this.view = 'loading';
    this.fatal = null;
    let boot: AppBootstrap;
    let list: readonly SessionSummary[];
    let status: Promise<unknown> = Promise.resolve();
    try {
      boot = await this.client.app.bootstrap();
      this.appVersion = boot.app_version;
      this.settings = { ...boot.settings };
      this.shortcutOverrides = { ...boot.shortcut_overrides };
      this.layout.load(boot.layout);
      await this.i18n.setLocale(boot.locale);
      this.unsubscribe?.();
      this.unsubscribe = this.client.subscribe((batch) => this.batcher.push(...batch));
      status = this.client.system.status().then(
        (value) => (this.system = value),
        (error: unknown) => this.loadFailed(error),
      );
      list = await this.client.sessions.list();
    } catch (error) {
      this.fatal = errorText(error);
      this.view = 'error';
      return;
    }
    this.sessions.list = [...list];
    void this.client.voice.status().then((v) => this.voice.applyStatus(v), logFailure('voice'));
    void this.client.gui.status().then((g) => this.work.applyGui(g), logFailure('gui'));
    void this.updates.load(this.client.updates);
    void this.broker.load(this.client.broker);
    const first = boot.active_session_id ?? list.find((s) => !s.archived)?.id ?? null;
    if (first) await this.openSession(first);
    else await this.refreshCosts().catch((error: unknown) => this.loadFailed(error));
    await status;
    this.view = boot.onboarding_done ? 'chat' : 'onboarding';
  }

  dispose(): void {
    this.unsubscribe?.();
    this.batcher.dispose();
    this.announcer.reset();
    this.debouncer.dispose();
  }

  /** Stosuje paczkę zdarzeń (najwyżej raz na klatkę). */
  applyBatch(batch: readonly AlfaEvent[]): void {
    for (const event of batch) applyEvent(this, event);
  }

  /** Zdarzenie strumienia do rozmowy sesji (jeśli jest w pamięci podręcznej). */
  applyChat(event: ChatStreamEvent): void {
    this.cache[event.session_id]?.apply(event);
  }

  /** Przekazuje zdarzenie panelom ładowanym leniwie. */
  notify(event: AlfaEvent): void {
    for (const listener of this.listeners) listener(event);
  }

  /** Cofa krok agentki (karta, toast, Replay) — przez dziennik cofania; `false` — nie cofnięto. */
  async undoStep(token: string, label: string): Promise<boolean> {
    const ok = await attempt(this.toasts, () => this.client.turns.undoStep(token));
    if (ok) {
      this.runs.markUndone(token);
      this.toasts.show({ kind: 'success', message: this.i18n.t('conv.undone', { label }) });
    }
    return ok;
  }

  /** Subskrypcje paneli ładowanych leniwie (oś czasu, Hub kont). */
  on(listener: (event: AlfaEvent) => void): () => void {
    this.listeners = [...this.listeners, listener];
    return () => {
      this.listeners = this.listeners.filter((l) => l !== listener);
    };
  }

  // ── Sesje ─────────────────────────────────────────────────────────────────────────────────────

  private conversationFor(id: string): ConversationState {
    let conv = this.cache[id];
    if (!conv) {
      conv = new ConversationState(this.client, id, {
        onText: (_turn, text) => {
          if (this.activeId === id) this.announcer.push(text);
        },
        onStop: () => {
          if (this.activeId === id) this.announcer.finish();
        },
        onError: (error) => showError(this.toasts, error),
      });
      this.cache[id] = conv;
    }
    this.cacheOrder = [id, ...this.cacheOrder.filter((x) => x !== id)];
    for (const old of this.cacheOrder.splice(CACHE_SIZE)) {
      if (!this.cache[old]?.streaming) delete this.cache[old];
    }
    return conv;
  }

  /** Nie odrzuca: błąd rozmowy pokazuje widok („Ponów"), reszty (koszty, agentki, szkic) — toast. */
  async openSession(id: string): Promise<void> {
    this.batcher.flushNow();
    this.announcer.reset();
    const conv = this.conversationFor(id);
    this.sessions.activeId = id;
    this.sessions.touch(id);
    this.conversation = conv;
    this.findOpen = false;
    this.timelineTurn = null;
    const loads: Promise<unknown>[] = [this.refreshCosts(), this.loadAgents(id)];
    if (!conv.loaded || conv.loadError) loads.push(conv.load());
    if (this.sessions.drafts[id] === undefined) {
      loads.push(this.client.sessions.getDraft(id).then((d) => (this.sessions.drafts[id] = d)));
    }
    const failed = (await Promise.allSettled(loads)).find((r) => r.status === 'rejected');
    // Szybkie przełączanie: spóźnione A nie może nadpisać aktywnej B w rdzeniu.
    if (this.activeId !== id) return;
    if (failed) this.loadFailed(failed.reason);
    this.client.app.setActiveSession(id).catch(logFailure('setActiveSession'));
    if (this.sessions.active?.unread)
      this.client.sessions.markRead(id).catch(logFailure('markRead'));
  }

  /** „Przejdź do sesji" spoza okna (zasobnik, `alfa://session/…`, Szybkie pytanie). */
  async focusSession(id: string): Promise<void> {
    if (!this.sessions.list.some((s) => s.id === id)) {
      try {
        this.sessions.list = [...(await this.client.sessions.list())];
      } catch (error) {
        this.loadFailed(error);
        return;
      }
    }
    if (this.view === 'settings') this.view = 'chat';
    await this.openSession(id);
  }

  /** Toast „Nie udało się wczytać: …" dla danych dociąganych w tle. */
  private loadFailed(error: unknown): void {
    this.toasts.show({
      kind: 'error',
      message: this.i18n.t('common.loadFailed', { error: errorText(error) }),
    });
  }

  private async loadAgents(id: string): Promise<void> {
    if (!this.agents[id]) this.agents[id] = [...(await this.client.agents.list(id))];
  }

  /** Koszty aktywnej sesji; odrzuca przy błędzie — wołający pokazuje go (start, otwarcie sesji). */
  async refreshCosts(): Promise<void> {
    const id = this.activeId;
    const costs = await this.client.costs.summary(id);
    if (this.activeId === id) this.costs = costs;
  }

  // Akcje na sesjach i ustawieniach same pokazują błąd i cofają zmiany (session-actions.ts,
  // settings-actions.ts); wynik `true` — sukces.

  newSession(template?: SessionTemplate): Promise<boolean> {
    return sessionActions.createSession(this, template);
  }

  renameSession(id: string, title: string): Promise<boolean> {
    return sessionActions.renameSession(this, id, title);
  }

  setPinned(id: string, pinned: boolean): Promise<boolean> {
    return sessionActions.setPinned(this, id, pinned);
  }

  setArchived(id: string, archived: boolean): Promise<boolean> {
    return sessionActions.setArchived(this, id, archived);
  }

  /** Usunięcie z cofnięciem przez 10 s (toast „Cofnij"). */
  deleteSession(id: string): Promise<boolean> {
    return sessionActions.deleteSession(this, id);
  }

  exportSession(id: string): Promise<boolean> {
    return sessionActions.exportSession(this, id);
  }

  /** Szkic sesji (domyślnie aktywnej; inna — np. przywrócenie po nieudanym wysłaniu). */
  setDraft(text: string, id: string | null = this.activeId): void {
    if (!id) return;
    this.sessions.drafts[id] = text;
    this.debouncer.run(`draft:${id}`, () => void this.drafts.run(id, text));
  }

  // ── Ustawienia i wygląd ───────────────────────────────────────────────────────────────────────

  str(key: string, fallback: string): string {
    const value = this.settings[key];
    return typeof value === 'string' ? value : fallback;
  }

  num(key: string, fallback: number): number {
    const value = this.settings[key];
    return typeof value === 'number' ? value : fallback;
  }

  bool(key: string, fallback: boolean): boolean {
    const value = this.settings[key];
    return typeof value === 'boolean' ? value : fallback;
  }

  setSetting(key: string, value: SettingValue): Promise<boolean> {
    return settingsActions.setSetting(this, key, value);
  }

  resetSetting(key: string): Promise<boolean> {
    return settingsActions.resetSetting(this, key);
  }

  setShortcut(actionId: string, chord: string | null): Promise<boolean> {
    return settingsActions.setShortcut(this, actionId, chord);
  }

  // ── Widok i panele ────────────────────────────────────────────────────────────────────────────

  openSettings(page?: string): void {
    if (page) this.settingsPage = page;
    this.view = 'settings';
  }

  closeSettings(): void {
    this.view = 'chat';
    this.hubWizard = false;
  }

  /** „Dodaj klucz, aby odblokować…" → Ustawienia › Modele i dostawcy z otwartym kreatorem. */
  addProviderKey(): void {
    this.hubWizard = true;
    this.openSettings('providers');
  }

  openPanel(tab: PanelId): void {
    this.layout.openTab(this.activeId, tab);
  }

  showTimelineFor(turnId: string): void {
    this.timelineTurn = turnId;
    this.openPanel('timeline');
  }

  /** Stop strumienia aktywnej rozmowy; błąd pokazuje toast (ConversationState, `onError`). */
  async stopGeneration(): Promise<boolean> {
    return (await this.conversation?.stop()) ?? true;
  }
}
