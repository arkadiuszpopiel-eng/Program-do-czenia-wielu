// Układ okna: szerokość efektywna (po zoomie), tryb responsywny, szerokości paneli (per maszyna),
// panel Sesje (wspólny — lista rozmów to nawigacja) i panel prawy (per sesja). Zapis przez
// `app.saveLayout` z opóźnieniem (debounce).
import type { LayoutPrefs, PanelId, SessionPanels } from '../api/types-system';
import {
  clampLeft,
  clampRight,
  layoutMode,
  resolvePanels,
  withPeek,
  type LayoutMode,
  type PanelLayout,
  type PanelSide,
  RIGHT_DEFAULT,
} from '../logic/layout';

/** Menu Ustawień schowane — zapamiętane w tym oknie (jak zwinięte grupy menu). */
const SETTINGS_NAV_KEY = 'alfa.settings.navHidden';

function readFlag(key: string): boolean {
  try {
    return globalThis.localStorage?.getItem(key) === '1';
  } catch {
    return false;
  }
}

function writeFlag(key: string, on: boolean): void {
  try {
    globalThis.localStorage?.setItem(key, on ? '1' : '0');
  } catch {
    // Brak pamięci okna (tryb prywatny, podgląd) — stan zostaje tylko do zamknięcia.
  }
}

export class LayoutState {
  /** Wygląd okna niezależny od trybu zgód i sposobu współpracy person. */
  cockpit = $state(readFlag('alfa.ui.cockpit'));
  toggleCockpit(): void {
    this.cockpit = !this.cockpit;
    writeFlag('alfa.ui.cockpit', this.cockpit);
  }
  /** Szerokość okna w px CSS po uwzględnieniu zoomu. */
  width = $state(1280);
  leftWidth = $state(264);
  rightWidth = $state(RIGHT_DEFAULT);
  /** Panel Sesje schowany na szerokim i średnim oknie (zapis: `left_collapsed`). */
  leftCollapsed = $state(false);
  /** Szuflada Sesji na wąskim oknie — chwilowa, nie zapisuje się. */
  leftDrawer = $state(false);
  focus = $state(false);
  panels = $state<Record<string, SessionPanels>>({});
  /** Ustawienie `ui.panels_autohide` (nie zapisuje się z układem). */
  autohide = $state(false);
  /** Panel wysunięty chwilowo nad rozmową (znacznik przy krawędzi). */
  peek = $state<PanelSide | null>(null);
  /** Menu Ustawień schowane za znacznik przy krawędzi (29.10). */
  settingsNavHidden = $state(readFlag(SETTINGS_NAV_KEY));
  /** Menu Ustawień wysunięte chwilowo nad stronę. */
  settingsPeek = $state(false);
  readonly mode: LayoutMode = $derived(layoutMode(this.width));

  constructor(private readonly persist: (prefs: LayoutPrefs) => void = () => undefined) {}

  /** Panele obok rozmowy (szerokie i średnie okno); węższe — szuflady i arkusze. */
  get docking(): boolean {
    return this.mode === 'wide' || this.mode === 'medium';
  }

  current(sessionId: string | null): SessionPanels {
    // Ten sam klucz co w `update`: bez aktywnej sesji (pusta instalacja, usunięta ostatnia
    // rozmowa) przełączniki paneli zapisywały stan, którego potem nikt nie czytał.
    // Bez rozmowy panel prawy nie ma czego pokazać — startuje schowany (znacznik przy krawędzi).
    const own = this.panels[sessionId ?? '$none'] ?? {
      left_open: true,
      right_open: this.mode === 'wide' && sessionId !== null,
      right_tab: 'agents' as const,
    };
    return { ...own, left_open: this.docking ? !this.leftCollapsed : this.leftDrawer };
  }

  placement(sessionId: string | null): PanelLayout {
    const p = this.current(sessionId);
    const base = resolvePanels(this.mode, p.left_open, p.right_open, this.focus);
    return withPeek(base, this.mode, this.focus, this.autohide, this.peek);
  }

  /** Czy panele wysuwają się nad rozmowę (szerokie i średnie okno poza trybem skupienia). */
  get peekable(): boolean {
    return !this.focus && (this.mode === 'wide' || this.mode === 'medium');
  }

  update(sessionId: string | null, patch: Partial<SessionPanels>): void {
    const key = sessionId ?? '$none';
    // Panele chowane: „otwórz” (skrót, karta z belki, szukanie) wysuwa panel nad rozmowę.
    if (this.autohide && this.peekable) {
      if (patch.left_open) this.peek = 'left';
      if (patch.right_open) this.peek = 'right';
    }
    // „Zamknij” w wysuniętym panelu chowa go, a „otwórz” bez chowania — dokuje.
    if (patch.left_open !== undefined && this.peek === 'left' && !this.autohide) this.peek = null;
    if (patch.right_open !== undefined && this.peek === 'right' && !this.autohide) this.peek = null;
    if (patch.left_open === false && this.peek === 'left') this.peek = null;
    if (patch.right_open === false && this.peek === 'right') this.peek = null;
    const next = { ...this.current(sessionId), ...patch };
    // Na wąskich ekranach naraz tylko jedna szuflada/arkusz.
    if (!this.docking) {
      if (patch.left_open) next.right_open = false;
      if (patch.right_open) next.left_open = false;
    }
    // Średnie okno mieści jeden panel: pokazanie Sesji zamyka prawy (inaczej przycisk nic nie
    // pokazywał — Sesje chowają się za znacznik, gdy otwarte są oba).
    if (this.mode === 'medium' && patch.left_open && !this.autohide) next.right_open = false;
    if (this.docking) this.leftCollapsed = !next.left_open;
    else this.leftDrawer = next.left_open;
    this.panels[key] = next;
    this.save();
  }

  /** Czy panel jest teraz widoczny (zadokowany, szuflada, arkusz albo wysunięty). */
  shown(sessionId: string | null, side: PanelSide): boolean {
    return this.placement(sessionId)[side] !== 'hidden';
  }

  toggleLeft(sessionId: string | null): void {
    if (this.autohide && this.peekable) return this.togglePeek('left');
    const placement = this.placement(sessionId).left;
    this.update(sessionId, { left_open: placement === 'hidden' || placement === 'peek' });
  }

  toggleRight(sessionId: string | null): void {
    if (this.autohide && this.peekable) return this.togglePeek('right');
    const placement = this.placement(sessionId).right;
    this.update(sessionId, { right_open: placement === 'hidden' || placement === 'peek' });
  }

  setSettingsNav(hidden: boolean): void {
    this.settingsNavHidden = hidden;
    this.settingsPeek = false;
    writeFlag(SETTINGS_NAV_KEY, hidden);
  }

  togglePeek(side: PanelSide): void {
    this.peek = this.peek === side ? null : side;
  }

  openTab(sessionId: string | null, tab: PanelId): void {
    this.update(sessionId, { right_open: true, right_tab: tab });
  }

  setLeftWidth(width: number): void {
    this.leftWidth = clampLeft(width);
    this.save();
  }

  setRightWidth(width: number): void {
    // Wartość domyślna znaczy „dopasuj do okna” (na szerokim oknie panel rośnie), więc ręcznie
    // ustawiona szerokość nie może być jej równa — inaczej panel przeskakiwał.
    const next = clampRight(width);
    this.rightWidth = next === RIGHT_DEFAULT ? RIGHT_DEFAULT + 1 : next;
    this.save();
  }

  /** Tryb skupienia (tylko rozmowa) — zapisywany z układem i przywracany po uruchomieniu. */
  setFocus(on: boolean): void {
    this.focus = on;
    this.save();
  }

  load(prefs: LayoutPrefs | null): void {
    if (!prefs) return;
    this.leftWidth = clampLeft(prefs.left_width);
    this.rightWidth = clampRight(prefs.right_width);
    this.leftCollapsed = prefs.left_collapsed;
    this.focus = prefs.focus;
    this.panels = { ...prefs.sessions };
  }

  toPrefs(): LayoutPrefs {
    return {
      left_width: this.leftWidth,
      right_width: this.rightWidth,
      left_collapsed: this.leftCollapsed,
      sessions: $state.snapshot(this.panels),
      focus: this.focus,
    };
  }

  private save(): void {
    this.persist(this.toPrefs());
  }
}
