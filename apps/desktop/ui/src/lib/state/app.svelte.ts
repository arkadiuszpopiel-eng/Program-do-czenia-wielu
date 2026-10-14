// Stan aplikacji okna głównego: start, zdarzenia (paczka raz na klatkę), sesje, rozmowa,
// ustawienia, stan systemu, koszty, agentki. Komponenty czytają pola, akcje wołają metody.
import type { CamerasState } from '../api/types-cameras';
import type { MicState } from '@alfa/ui-kit';
import type { AlfaClient } from '../api/client';
import type { ActivityInfo, AgentState, CostSummary, SessionTemplate } from '../api/types';
import type {
  AlfaEvent,
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
import { showError } from './attempt';
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
import { NavHistory, type Place } from '../logic/nav-history';
import * as flow from './app-flow';

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
  /** Konto wybrane na stronie „Kolejki modeli API” (ustawiane przez przejście z listy kont). */
  apiAccount = $state('');
  micLevel = $state(0);
  /** Kamery: lista i stan (`null`, dopóki nie wczytano). */
  cameras = $state.raw<CamerasState | null>(null);
  /** Agentka, której mowę właśnie słychać z głośnika (z pomiaru poziomu); pusty = nikt. */
  speechAgent = $state('');
  private speechTimer: ReturnType<typeof setTimeout> | undefined;
  /** Surowy poziom mikrofonu z testu (przed redukcją szumów); `null` poza testem. */
  micRaw = $state<number | null>(null);
  conversation = $state.raw<ConversationState | null>(null);
  palette = $state<{ open: boolean; mode: 'all' | 'sessions' }>({ open: false, mode: 'all' });
  cheatsheetOpen = $state(false);
  findOpen = $state(false);
  settingsPage = $state('general');
  /** Odwiedzone miejsca dla „Wstecz / Dalej” (Alt+←, Alt+→, boczne przyciski myszy). */
  readonly nav = new NavHistory();
  /** Trwa przejście po historii — pośrednie stany nie są zapisywane jako nowe miejsca. */
  navBusy = false;
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
  /** Opis bieżącego kroku startu — pokazywany na ekranie ładowania. */
  bootStep = $state('');
  /** Ostatni postęp pobierania modelu lokalnego (onboarding, Ustawienia). */
  localDownload = $state<LocalModelProgress | null>(null);

  // Pola bez `private` niżej (batcher, announcer, cache, removing, navBusy, unsubscribe,
  // conversationFor, loadFailed, loadAgents) są wewnętrzne — używa ich tylko `app-flow.ts`.
  readonly keymap = $derived(buildKeymap(SHORTCUTS, this.shortcutOverrides));
  readonly bindings = $derived(effectiveBindings(SHORTCUTS, this.shortcutOverrides));

  readonly batcher: RafBatcher<AlfaEvent>;
  readonly announcer: SentenceAnnouncer;
  readonly cache: Record<string, ConversationState> = {};
  cacheOrder: string[] = [];
  unsubscribe: (() => void) | null = null;
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

  /** Pomiar poziomu mowy: agentka „mówi”, dopóki przychodzą dźwięki; krótka pauza jej nie gasi. */
  noteSpeech(agent: string, level: number): void {
    if (level <= 0 || !agent) return;
    this.speechAgent = agent;
    clearTimeout(this.speechTimer);
    this.speechTimer = setTimeout(() => (this.speechAgent = ''), 900);
  }

  /**
   * Start okna. Krytyczne (widok błędu z „Ponów"): ustawienia startowe, słownik, lista sesji.
   * Reszta (stan systemu, głos, koszty, agentki, szkic) — toast albo dziennik, okno działa dalej.
   */
  async start(): Promise<void> {
    return flow.start(this);
  }

  /**
   * Zdarzenia przepadły i rdzeń nie umie ich już odtworzyć: stan z rdzenia od nowa, bez zmiany
   * widoku. Rozmowy w pamięci podręcznej poza otwartą są porzucane (wczytają się przy otwarciu).
   */
  async resync(): Promise<void> {
    return flow.resync(this);
  }

  dispose(): void {
    this.unsubscribe?.();
    this.batcher.dispose();
    this.announcer.reset();
    this.debouncer.dispose();
  }

  /** Stosuje paczkę zdarzeń (najwyżej raz na klatkę). */
  applyBatch(batch: readonly AlfaEvent[]): void {
    // Jedno wadliwe zdarzenie nie może zgubić reszty paczki — wśród nich bywa `Stop`, bez którego
    // tura zostałaby na stałe w stanie „pisze".
    for (const event of batch) {
      try {
        applyEvent(this, event);
      } catch (error) {
        logFailure(`zdarzenie ${event.type}`)(error);
      }
    }
  }

  /** Sprzątanie po usuniętej sesji (także usuniętej poza tym oknem): stan per sesja i widok. */
  async sessionRemoved(id: string): Promise<void> {
    return flow.sessionRemoved(this, id);
  }

  /** Sesje w trakcie sprzątania (zwykła lista — nic w widoku od niej nie zależy). */
  removing: string[] = [];

  async forgetSession(id: string): Promise<void> {
    return flow.forgetSession(this, id);
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
    return flow.undoStep(this, token, label);
  }

  /** Subskrypcje paneli ładowanych leniwie (oś czasu, Hub kont). */
  on(listener: (event: AlfaEvent) => void): () => void {
    this.listeners = [...this.listeners, listener];
    return () => {
      this.listeners = this.listeners.filter((l) => l !== listener);
    };
  }

  // ── Sesje ─────────────────────────────────────────────────────────────────────────────────────

  conversationFor(id: string): ConversationState {
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
      // Rozmowa w trakcie strumienia zostaje w kolejce — inaczej po jego końcu nikt by jej nie zwolnił.
      if (this.cache[old]?.streaming) this.cacheOrder.push(old);
      else delete this.cache[old];
    }
    return conv;
  }

  /** Nie odrzuca: błąd rozmowy pokazuje widok („Ponów"), reszty (koszty, agentki, szkic) — toast. */
  async openSession(id: string): Promise<void> {
    return flow.openSession(this, id);
  }

  /** „Przejdź do sesji" spoza okna (zasobnik, `alfa://session/…`, Szybkie pytanie). */
  async focusSession(id: string): Promise<void> {
    return flow.focusSession(this, id);
  }

  /** Toast „Nie udało się wczytać: …" dla danych dociąganych w tle. */
  loadFailed(error: unknown): void {
    this.toasts.show({
      kind: 'error',
      message: this.i18n.t('common.loadFailed', { error: errorText(error) }),
    });
  }

  async loadAgents(id: string): Promise<void> {
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
    return typeof this.settings[key] === 'string' ? (this.settings[key] as string) : fallback;
  }

  num(key: string, fallback: number): number {
    return typeof this.settings[key] === 'number' ? (this.settings[key] as number) : fallback;
  }

  bool(key: string, fallback: boolean): boolean {
    return typeof this.settings[key] === 'boolean' ? (this.settings[key] as boolean) : fallback;
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

  // ── Wstecz / Dalej ────────────────────────────────────────────────────────────────────────────

  /** Bieżące miejsce (rozmowa z sesją albo strona Ustawień); `null` poza nimi (start, wprowadzenie). */
  place(): Place | null {
    if (this.view === 'chat') return { view: 'chat', page: null, session: this.activeId };
    if (this.view === 'settings')
      return { view: 'settings', page: this.settingsPage, session: this.activeId };
    return null;
  }

  /** Zapis odwiedzonego miejsca (wołany po każdej zmianie widoku, strony albo sesji). */
  recordPlace(place: Place | null): void {
    if (place && !this.navBusy) this.nav.record(place);
  }

  /** Krok wstecz (`-1`) albo dalej (`1`) po odwiedzonych miejscach; bez historii — tylko komunikat. */
  async navigate(direction: -1 | 1): Promise<void> {
    return flow.navigate(this, direction);
  }

  // ── Widok i panele ────────────────────────────────────────────────────────────────────────────

  openSettings(page?: string): void {
    if (page) this.settingsPage = page;
    this.view = 'settings';
  }

  /** Strona „Kolejki modeli API” z wybranym kontem (przycisk „Modele” przy koncie). */
  openApiFor(accountId: string): void {
    this.apiAccount = accountId;
    this.openSettings('api');
  }

  closeSettings(): void {
    this.view = 'chat';
    this.hubWizard = false;
  }

  /** „Dodaj klucz, aby odblokować…" → Ustawienia › Konta i klucze API z otwartym kreatorem. */
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
