// Stan aplikacji okna głównego: start, zdarzenia (paczka raz na klatkę), sesje, rozmowa,
// ustawienia, stan systemu, koszty, agentki. Komponenty czytają pola, akcje wołają metody.
import type { MicState } from '@alfa/ui-kit';
import type { AlfaClient } from '../api/client';
import type { ActivityInfo, AgentState, CostSummary, SessionTemplate } from '../api/types';
import type { AlfaEvent, PanelId, SettingValue, SystemStatus } from '../api/types-system';

/** Zdarzenie postępu pobierania modelu lokalnego. */
export type LocalModelProgress = Extract<AlfaEvent, { type: 'LocalModelProgress' }>;
import { i18n, type I18n } from '../i18n/i18n.svelte';
import { isChatEvent } from '../logic/apply-event';
import { RafBatcher, type FrameScheduler } from '../logic/raf-batcher';
import { SentenceAnnouncer, type Timers } from '../logic/sentence-announcer';
import { SHORTCUTS } from '../logic/shortcut-registry';
import { buildKeymap, effectiveBindings } from '../logic/shortcuts';
import { ConversationState } from './conversation.svelte';
import { LayoutState } from './layout.svelte';
import { SessionsState } from './sessions.svelte';
import { ToastState } from './toasts.svelte';

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
  private readonly debounceMs: number;
  private readonly pending: Record<string, ReturnType<typeof setTimeout>> = {};

  constructor(
    readonly client: AlfaClient,
    options: AppOptions = {},
  ) {
    this.debounceMs = options.debounceMs ?? 400;
    this.layout = new LayoutState((prefs) =>
      this.debounce('layout', () => void this.client.app.saveLayout(prefs)),
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

  async start(): Promise<void> {
    try {
      const boot = await this.client.app.bootstrap();
      this.appVersion = boot.app_version;
      this.settings = { ...boot.settings };
      this.shortcutOverrides = { ...boot.shortcut_overrides };
      this.layout.load(boot.layout);
      await this.i18n.setLocale(boot.locale);
      this.unsubscribe = this.client.subscribe((batch) => this.batcher.push(...batch));
      const [list, status] = await Promise.all([
        this.client.sessions.list(),
        this.client.system.status(),
      ]);
      this.sessions.list = [...list];
      this.system = status;
      const first = boot.active_session_id ?? list.find((s) => !s.archived)?.id ?? null;
      if (first) await this.openSession(first);
      else await this.refreshCosts();
      this.view = boot.onboarding_done ? 'chat' : 'onboarding';
    } catch (error) {
      this.fatal = error instanceof Error ? error.message : String(error);
      this.view = 'error';
    }
  }

  dispose(): void {
    this.unsubscribe?.();
    this.batcher.dispose();
    this.announcer.reset();
    for (const handle of Object.values(this.pending)) clearTimeout(handle);
  }

  /** Stosuje paczkę zdarzeń (najwyżej raz na klatkę). */
  applyBatch(batch: readonly AlfaEvent[]): void {
    for (const event of batch) {
      switch (event.type) {
        case 'SessionUpdated':
          this.sessions.upsert(event.session);
          break;
        case 'SessionRemoved':
          this.sessions.remove(event.session_id);
          break;
        case 'AgentsChanged':
          this.agents[event.session_id] = [...event.agents];
          break;
        case 'ActivityChanged':
          this.activity[event.session_id] = event.activity;
          break;
        case 'CostsChanged':
          if (event.session_id === this.activeId || !event.session_id) this.costs = event.costs;
          break;
        case 'SystemStatusChanged':
          this.system = event.status;
          break;
        case 'MicLevel':
          this.micLevel = event.level;
          break;
        case 'VoicePill':
          this.micState = event.state.mic;
          break;
        case 'Toast':
          this.toasts.show({ kind: event.kind, message: this.i18n.text(event.message) });
          break;
        case 'OpenSession':
          void this.focusSession(event.session_id);
          break;
        case 'LocalModelProgress':
          this.localDownload = event;
          break;
        case 'TimelineAppended':
        case 'AccountChanged':
          for (const listener of this.listeners) listener(event);
          break;
        default:
          if (isChatEvent(event)) this.cache[event.session_id]?.apply(event);
      }
    }
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
      });
      this.cache[id] = conv;
    }
    this.cacheOrder = [id, ...this.cacheOrder.filter((x) => x !== id)];
    for (const old of this.cacheOrder.splice(CACHE_SIZE)) {
      if (!this.cache[old]?.streaming) delete this.cache[old];
    }
    return conv;
  }

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
    if (!conv.loaded) loads.push(conv.load());
    if (this.sessions.drafts[id] === undefined) {
      loads.push(this.client.sessions.getDraft(id).then((d) => (this.sessions.drafts[id] = d)));
    }
    await Promise.all(loads);
    void this.client.app.setActiveSession(id);
    if (this.sessions.active?.unread) void this.client.sessions.markRead(id);
  }

  /** „Przejdź do sesji" spoza okna (zasobnik, `alfa://session/…`, Szybkie pytanie). */
  async focusSession(id: string): Promise<void> {
    if (!this.sessions.list.some((s) => s.id === id)) {
      this.sessions.list = [...(await this.client.sessions.list())];
    }
    if (this.view === 'settings') this.view = 'chat';
    await this.openSession(id);
  }

  private async loadAgents(id: string): Promise<void> {
    if (!this.agents[id]) this.agents[id] = [...(await this.client.agents.list(id))];
  }

  async refreshCosts(): Promise<void> {
    this.costs = await this.client.costs.summary(this.activeId);
  }

  async newSession(template?: SessionTemplate): Promise<void> {
    const chosen = template ?? (this.str('sessions.default_template', 'empty') as SessionTemplate);
    const created = await this.client.sessions.create(chosen);
    this.sessions.upsert(created);
    this.view = 'chat';
    await this.openSession(created.id);
  }

  async renameSession(id: string, title: string): Promise<void> {
    const trimmed = title.trim();
    this.sessions.renamingId = null;
    const current = this.sessions.list.find((s) => s.id === id);
    if (!trimmed || !current || current.title === trimmed) return;
    this.sessions.upsert({ ...current, title: trimmed });
    await this.client.sessions.rename(id, trimmed);
  }

  async setPinned(id: string, pinned: boolean): Promise<void> {
    await this.client.sessions.setPinned(id, pinned);
  }

  async setArchived(id: string, archived: boolean): Promise<void> {
    await this.client.sessions.setArchived(id, archived);
  }

  /** Usunięcie z cofnięciem przez 10 s (toast „Cofnij"). */
  async deleteSession(id: string): Promise<void> {
    const session = this.sessions.list.find((s) => s.id === id);
    const ticket = await this.client.sessions.remove(id);
    this.sessions.remove(id);
    if (this.activeId === id) {
      const next = this.sessions.list.find((s) => !s.archived);
      if (next) await this.openSession(next.id);
      else {
        this.sessions.activeId = null;
        this.conversation = null;
      }
    }
    this.toasts.show({
      kind: 'info',
      message: this.i18n.t('sessions.deleted', { title: session?.title ?? '' }),
      actionLabel: this.i18n.t('common.undo'),
      timeoutMs: 10_000,
      onAction: () => void this.client.sessions.undoRemove(ticket.token),
    });
  }

  async exportSession(id: string): Promise<void> {
    const result = await this.client.sessions.exportSession(id);
    if (result.status === 'saved') {
      this.toasts.show({
        kind: 'success',
        message: this.i18n.t('sessions.exported', { path: result.path }),
      });
    }
  }

  setDraft(text: string): void {
    const id = this.activeId;
    if (!id) return;
    this.sessions.drafts[id] = text;
    this.debounce(`draft:${id}`, () => void this.client.sessions.saveDraft(id, text));
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

  async setSetting(key: string, value: SettingValue): Promise<void> {
    this.settings[key] = value;
    if (key === 'ui.locale' && (value === 'pl' || value === 'en')) await this.i18n.setLocale(value);
    await this.client.settings.set(key, value);
  }

  async resetSetting(key: string): Promise<void> {
    const value = await this.client.settings.reset(key);
    this.settings[key] = value;
    if (key === 'ui.locale' && (value === 'pl' || value === 'en')) await this.i18n.setLocale(value);
  }

  async setShortcut(actionId: string, chord: string | null): Promise<void> {
    if (chord === null) delete this.shortcutOverrides[actionId];
    else this.shortcutOverrides[actionId] = chord;
    await this.client.settings.setShortcut(actionId, chord);
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

  async stopGeneration(): Promise<void> {
    await this.conversation?.stop();
  }

  private debounce(key: string, run: () => void): void {
    const existing = this.pending[key];
    if (existing) clearTimeout(existing);
    if (this.debounceMs <= 0) {
      run();
      return;
    }
    this.pending[key] = setTimeout(() => {
      delete this.pending[key];
      run();
    }, this.debounceMs);
  }
}
