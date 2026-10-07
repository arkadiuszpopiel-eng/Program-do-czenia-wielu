import { describe, expect, it } from 'vitest';
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

  it('odczyt i zapis preferencji', () => {
    const l = new LayoutState();
    l.load({
      left_width: 300,
      right_width: 400,
      left_collapsed: true,
      sessions: { s: { left_open: false, right_open: true, right_tab: 'timeline' } },
    });
    expect(l.toPrefs()).toEqual({
      left_width: 300,
      right_width: 400,
      left_collapsed: true,
      sessions: { s: { left_open: false, right_open: true, right_tab: 'timeline' } },
    });
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
});
