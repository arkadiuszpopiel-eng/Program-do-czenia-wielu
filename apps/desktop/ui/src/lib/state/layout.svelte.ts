// Układ okna: szerokość efektywna (po zoomie), tryb responsywny, szerokości paneli (per maszyna)
// i otwarte panele (per sesja). Zapis przez `app.saveLayout` z opóźnieniem (debounce).
import type { LayoutPrefs, PanelId, SessionPanels } from '../api/types-system';
import {
  clampLeft,
  clampRight,
  layoutMode,
  resolvePanels,
  type LayoutMode,
  type PanelLayout,
} from '../logic/layout';

export class LayoutState {
  /** Szerokość okna w px CSS po uwzględnieniu zoomu. */
  width = $state(1280);
  leftWidth = $state(264);
  rightWidth = $state(360);
  leftCollapsed = $state(false);
  focus = $state(false);
  panels = $state<Record<string, SessionPanels>>({});
  readonly mode: LayoutMode = $derived(layoutMode(this.width));

  constructor(private readonly persist: (prefs: LayoutPrefs) => void = () => undefined) {}

  current(sessionId: string | null): SessionPanels {
    return (
      (sessionId ? this.panels[sessionId] : undefined) ?? {
        left_open: this.mode !== 'compact' && this.mode !== 'narrow',
        right_open: this.mode === 'wide',
        right_tab: 'agents',
      }
    );
  }

  placement(sessionId: string | null): PanelLayout {
    const p = this.current(sessionId);
    return resolvePanels(this.mode, p.left_open, p.right_open, this.focus);
  }

  update(sessionId: string | null, patch: Partial<SessionPanels>): void {
    const key = sessionId ?? '$none';
    const next = { ...this.current(sessionId), ...patch };
    // Na wąskich ekranach naraz tylko jedna szuflada/arkusz.
    if (this.mode === 'narrow' || this.mode === 'compact') {
      if (patch.left_open) next.right_open = false;
      if (patch.right_open) next.left_open = false;
    }
    this.panels[key] = next;
    this.save();
  }

  toggleLeft(sessionId: string | null): void {
    this.update(sessionId, { left_open: !this.current(sessionId).left_open });
  }

  toggleRight(sessionId: string | null): void {
    this.update(sessionId, { right_open: !this.current(sessionId).right_open });
  }

  openTab(sessionId: string | null, tab: PanelId): void {
    this.update(sessionId, { right_open: true, right_tab: tab });
  }

  setLeftWidth(width: number): void {
    this.leftWidth = clampLeft(width);
    this.save();
  }

  setRightWidth(width: number): void {
    this.rightWidth = clampRight(width);
    this.save();
  }

  setLeftCollapsed(collapsed: boolean): void {
    this.leftCollapsed = collapsed;
    this.save();
  }

  load(prefs: LayoutPrefs | null): void {
    if (!prefs) return;
    this.leftWidth = clampLeft(prefs.left_width);
    this.rightWidth = clampRight(prefs.right_width);
    this.leftCollapsed = prefs.left_collapsed;
    this.panels = { ...prefs.sessions };
  }

  toPrefs(): LayoutPrefs {
    return {
      left_width: this.leftWidth,
      right_width: this.rightWidth,
      left_collapsed: this.leftCollapsed,
      sessions: $state.snapshot(this.panels),
    };
  }

  private save(): void {
    this.persist(this.toPrefs());
  }
}
