import { describe, expect, it, vi } from 'vitest';
import { session } from '../../api/fake/fixtures';
import { LayoutState } from '../layout.svelte';
import { SessionsState, groupSessions } from '../sessions.svelte';
import { ToastState, type ToastTimers } from '../toasts.svelte';

const T = Date.UTC(2026, 8, 30, 12);

describe('SessionsState', () => {
  const list = [
    session('a', 'A', T - 1000, { pinned: true }),
    session('b', 'B', T - 2000, { project: { id: 'p', name: 'Projekt' } }),
    session('c', 'C', T - 500),
    session('d', 'D', T - 100, { archived: true }),
  ];

  it('grupy: przypięte, projekty, bez projektu; archiwum na żądanie', () => {
    expect(groupSessions(list, false).map((g) => [g.kind, g.sessions.map((s) => s.id)])).toEqual([
      ['pinned', ['a']],
      ['project', ['b']],
      ['loose', ['c']],
    ]);
    expect(
      groupSessions(list, true)
        .at(-1)
        ?.sessions.map((s) => s.id),
    ).toEqual(['d']);
  });

  it('karty = ostatnio używane: Ctrl+1…9, Ctrl+Tab, Ctrl+W, Ctrl+Shift+T', () => {
    const s = new SessionsState();
    s.list = [...list];
    s.touch('c');
    s.touch('a');
    s.activeId = 'a';
    expect(s.tab(1)).toBe('a');
    expect(s.tab(2)).toBe('c');
    expect(s.cycle(1)).toBe('c');
    expect(s.cycle(-1)).toBe('b');
    expect(s.close('a')).toBe('c');
    expect(s.reopen()).toBe('a');
    expect(s.reopen()).toBeUndefined();
  });

  it('upsert i remove', () => {
    const s = new SessionsState();
    s.upsert(session('x', 'X', T));
    s.upsert(session('x', 'X2', T));
    expect(s.list.map((x) => x.title)).toEqual(['X2']);
    s.remove('x');
    expect(s.list).toEqual([]);
  });
});

describe('LayoutState', () => {
  it('szerokości w granicach i zapis', () => {
    const saved: unknown[] = [];
    const l = new LayoutState((p) => saved.push(p));
    l.setLeftWidth(1000);
    l.setRightWidth(100);
    expect([l.leftWidth, l.rightWidth]).toEqual([320, 300]);
    expect(saved).toHaveLength(2);
  });

  it('panele per sesja; na wąskim oknie tylko jedna szuflada', () => {
    const l = new LayoutState();
    l.width = 1600;
    l.update('s1', { right_open: true, right_tab: 'files' });
    expect(l.current('s1')).toMatchObject({
      left_open: true,
      right_open: true,
      right_tab: 'files',
    });
    expect(l.current('s2').right_tab).toBe('agents');
    l.width = 900;
    l.update('s1', { left_open: true });
    expect(l.current('s1').right_open).toBe(false);
    expect(l.placement('s1')).toEqual({ left: 'drawer', right: 'hidden' });
    l.focus = true;
    expect(l.placement('s1')).toEqual({ left: 'hidden', right: 'hidden' });
  });

  it('laptop (1280 px): jeden panel obok rozmowy, drugi za znacznikiem (bez paska ikon)', () => {
    const l = new LayoutState();
    l.width = 1280;
    l.update('s1', { right_open: true });
    expect(l.placement('s1')).toEqual({ left: 'hidden', right: 'docked' });
    // Przycisk Sesji nie może być martwy: pokazanie listy zamyka prawy panel.
    l.toggleLeft('s1');
    expect(l.placement('s1')).toEqual({ left: 'docked', right: 'hidden' });
    // Szerokie okno: oba obok siebie; schowane Sesje są wspólne dla wszystkich rozmów.
    l.width = 1600;
    l.update('s1', { right_open: true });
    expect(l.placement('s1')).toEqual({ left: 'docked', right: 'docked' });
    l.toggleLeft('s1');
    expect(l.leftCollapsed).toBe(true);
    expect(l.placement('s2').left).toBe('hidden');
    l.toggleLeft('s2');
    expect(l.placement('s1')).toEqual({ left: 'docked', right: 'docked' });
  });

  it('wysunięty panel: przełącznik go dokuje, a wąskie okno ma osobną szufladę Sesji', () => {
    const l = new LayoutState();
    l.width = 1600;
    l.update('s1', { left_open: false });
    l.peek = 'left';
    expect(l.placement('s1').left).toBe('peek');
    l.toggleLeft('s1');
    expect(l.peek).toBeNull();
    expect(l.placement('s1').left).toBe('docked');
    // Szuflada na wąskim oknie nie zmienia stanu szerokiego okna.
    l.width = 900;
    expect(l.placement('s1').left).toBe('hidden');
    l.toggleLeft('s1');
    expect(l.placement('s1').left).toBe('drawer');
    expect(l.leftCollapsed).toBe(false);
  });

  it('menu Ustawień: schowanie zapamiętane, przypięcie kończy wysunięcie', () => {
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
    });
    const l = new LayoutState();
    l.setSettingsNav(true);
    expect(new LayoutState().settingsNavHidden).toBe(true);
    l.settingsPeek = true;
    l.setSettingsNav(false);
    expect(l.settingsPeek).toBe(false);
    expect(new LayoutState().settingsNavHidden).toBe(false);
    vi.unstubAllGlobals();
  });

  it('odczyt i zapis preferencji', () => {
    const l = new LayoutState();
    l.load({
      left_width: 300,
      right_width: 400,
      left_collapsed: true,
      sessions: { s: { left_open: false, right_open: true, right_tab: 'timeline' } },
      focus: true,
    });
    expect(l.focus).toBe(true);
    expect(l.toPrefs()).toEqual({
      left_width: 300,
      right_width: 400,
      left_collapsed: true,
      sessions: { s: { left_open: false, right_open: true, right_tab: 'timeline' } },
      focus: true,
    });
    const saved: unknown[] = [];
    const persisted = new LayoutState((p) => saved.push(p));
    persisted.load({
      left_width: 300,
      right_width: 400,
      left_collapsed: false,
      sessions: {},
      focus: false,
    });
    expect(persisted.focus).toBe(false);
    persisted.setFocus(true);
    expect(saved).toEqual([expect.objectContaining({ focus: true })]);
  });
});

describe('ToastState', () => {
  class Timers implements ToastTimers {
    tasks = new Map<number, () => void>();
    id = 0;
    setTimeout = (fn: () => void) => {
      this.tasks.set(++this.id, fn);
      return this.id;
    };
    clearTimeout = (h: unknown) => void this.tasks.delete(h as number);
    fire() {
      for (const [id, fn] of [...this.tasks]) {
        this.tasks.delete(id);
        fn();
      }
    }
  }

  it('znika po czasie; wstrzymanie przy najechaniu; akcja', () => {
    const timers = new Timers();
    const toasts = new ToastState(timers);
    let undone = 0;
    const a = toasts.show({ kind: 'info', message: 'a' });
    const b = toasts.show({
      kind: 'info',
      message: 'b',
      actionLabel: 'Cofnij',
      onAction: () => undone++,
    });
    toasts.pause(a);
    timers.fire();
    expect(toasts.items.map((t) => t.id)).toEqual([a]);
    toasts.resume(a);
    timers.fire();
    expect(toasts.items).toEqual([]);
    toasts.act(b);
    expect(undone).toBe(0);
    const c = toasts.show({ kind: 'info', message: 'c', onAction: () => undone++ });
    toasts.act(c);
    expect(undone).toBe(1);
  });

  it('toast z tym samym kluczem zastępuje poprzedni (bez stosu jednakowych komunikatów)', () => {
    const timers = new Timers();
    const toasts = new ToastState(timers);
    const other = toasts.show({ kind: 'info', message: 'inny' });
    const first = toasts.show({ kind: 'success', message: 'zalogowano', key: 'bridge-login:a' });
    toasts.show({ kind: 'success', message: 'zalogowano', key: 'bridge-login:a' });
    const last = toasts.show({ kind: 'success', message: 'zalogowano', key: 'bridge-login:a' });
    const b = toasts.show({ kind: 'success', message: 'drugi most', key: 'bridge-login:b' });
    expect(toasts.items.map((t) => t.id)).toEqual([other, last, b]);
    // Zastąpiony toast nie zostawia odliczania, które zdjęłoby go „po czasie”.
    expect(timers.tasks.size).toBe(3);
    expect(toasts.items.some((t) => t.id === first)).toBe(false);
  });
});
