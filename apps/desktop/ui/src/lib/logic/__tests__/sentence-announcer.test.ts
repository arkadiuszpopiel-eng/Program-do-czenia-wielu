import { describe, expect, it } from 'vitest';
import { SentenceAnnouncer, stripMarkdown, type Timers } from '../sentence-announcer';

class FakeTimers implements Timers {
  time = 0;
  private tasks: { at: number; fn: () => void; id: number }[] = [];
  private id = 0;
  now = () => this.time;
  setTimeout = (fn: () => void, ms: number) => {
    const id = ++this.id;
    this.tasks.push({ at: this.time + ms, fn, id });
    return id;
  };
  clearTimeout = (handle: unknown) => {
    this.tasks = this.tasks.filter((t) => t.id !== handle);
  };
  advance(ms: number) {
    this.time += ms;
    const due = this.tasks.filter((t) => t.at <= this.time);
    this.tasks = this.tasks.filter((t) => t.at > this.time);
    for (const t of due) t.fn();
  }
}

describe('SentenceAnnouncer — aria-live na końcu zdania z throttlingiem', () => {
  it('ogłasza dopiero pełne zdanie', () => {
    const timers = new FakeTimers();
    const out: string[] = [];
    const a = new SentenceAnnouncer((t) => out.push(t), { timers, minIntervalMs: 1000 });
    a.push('Sprawdziłam sumy');
    expect(out).toEqual([]);
    a.push(' kontrolne. Różnica');
    expect(out).toEqual(['Sprawdziłam sumy kontrolne.']);
  });

  it('grupuje zdania w oknie throttlingu', () => {
    const timers = new FakeTimers();
    const out: string[] = [];
    const a = new SentenceAnnouncer((t) => out.push(t), { timers, minIntervalMs: 1000 });
    a.push('Pierwsze. ');
    a.push('Drugie! ');
    a.push('Trzecie? ');
    expect(out).toEqual(['Pierwsze.']);
    timers.advance(999);
    expect(out).toHaveLength(1);
    timers.advance(1);
    expect(out).toEqual(['Pierwsze.', 'Drugie! Trzecie?']);
  });

  it('finish ogłasza resztę natychmiast, bez znaczników Markdown', () => {
    const timers = new FakeTimers();
    const out: string[] = [];
    const a = new SentenceAnnouncer((t) => out.push(t), { timers });
    a.push('**Marża** wzrosła o `2,4` p.p');
    a.finish();
    expect(out).toEqual(['Marża wzrosła o 2,4 p.p']);
  });

  it('blok kodu zastępuje krótką informacją (także ogrodzenie rozcięte między deltami)', () => {
    const timers = new FakeTimers();
    const out: string[] = [];
    const a = new SentenceAnnouncer((t) => out.push(t), {
      timers,
      minIntervalMs: 0,
      codeLabel: 'Kod.',
    });
    a.push('Oto funkcja: `');
    a.push('``ts\nconst x = 1;\n``');
    a.push('`\nGotowe. ');
    a.finish();
    expect(out.join(' ')).toBe('Oto funkcja: Kod. Gotowe.');
  });

  it('stripMarkdown usuwa nagłówki, listy i linki', () => {
    expect(stripMarkdown('## Plan\n- [raport](http://x) *pilne*')).toBe('Plan raport pilne');
  });
});
