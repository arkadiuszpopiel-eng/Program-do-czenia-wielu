import { describe, expect, it } from 'vitest';
import { NavHistory, samePlace, type Place } from '../nav-history';

const chat = (session: string): Place => ({ view: 'chat', page: null, session });
const settings = (page: string, session = 's1'): Place => ({ view: 'settings', page, session });

describe('historia nawigacji', () => {
  it('pusta historia nie ma dokąd wracać', () => {
    const h = new NavHistory();
    expect([h.canBack, h.canForward, h.current, h.back(), h.forward()]).toEqual([
      false,
      false,
      null,
      null,
      null,
    ]);
  });

  it('wstecz i dalej chodzą po odwiedzonych miejscach, to samo miejsce nie jest dopisywane', () => {
    const h = new NavHistory();
    expect(h.record(chat('s1'))).toBe(true);
    expect(h.record(chat('s1'))).toBe(false);
    h.record(settings('voice'));
    h.record(settings('devices'));
    expect(h.length).toBe(3);
    expect(h.back()).toEqual(settings('voice'));
    expect(h.back()).toEqual(chat('s1'));
    expect(h.back()).toBeNull();
    expect(h.canBack).toBe(false);
    expect(h.forward()).toEqual(settings('voice'));
    expect(h.forward()).toEqual(settings('devices'));
    expect(h.forward()).toBeNull();
  });

  it('nowe miejsce po cofnięciu ucina gałąź „Dalej”', () => {
    const h = new NavHistory();
    for (const p of [chat('s1'), settings('voice'), settings('devices')]) h.record(p);
    h.back();
    h.record(settings('models'));
    expect(h.canForward).toBe(false);
    expect(h.back()).toEqual(settings('voice'));
    expect(h.forward()).toEqual(settings('models'));
  });

  it('zapis miejsca, na które właśnie cofnięto, niczego nie zmienia', () => {
    const h = new NavHistory();
    for (const p of [chat('s1'), settings('voice')]) h.record(p);
    const place = h.back();
    expect(place && h.record(place)).toBe(false);
    expect(h.canForward).toBe(true);
  });

  it('trzyma najwyżej zadaną liczbę miejsc', () => {
    const h = new NavHistory(5);
    for (let i = 0; i < 12; i += 1) h.record(chat(`s${i}`));
    expect(h.length).toBe(5);
    expect(h.current).toEqual(chat('s11'));
    let steps = 0;
    while (h.back()) steps += 1;
    expect(steps).toBe(4);
    expect(h.current).toEqual(chat('s7'));
  });

  it('usunięta sesja znika z historii, a sąsiednie duplikaty są scalane', () => {
    const h = new NavHistory();
    for (const p of [chat('s1'), chat('s2'), chat('s1'), settings('voice', 's1')]) h.record(p);
    h.prune((p) => p.session !== 's2');
    expect(h.length).toBe(2);
    expect(h.current).toEqual(settings('voice', 's1'));
    expect(h.back()).toEqual(chat('s1'));
    expect(samePlace(chat('a'), chat('a'))).toBe(true);
    expect(samePlace(chat('a'), settings('voice', 'a'))).toBe(false);
  });
});
