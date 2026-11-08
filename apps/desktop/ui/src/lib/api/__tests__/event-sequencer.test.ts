import { describe, expect, it } from 'vitest';
import { EventSequencer } from '../event-sequencer';
import type { AlfaEvent, EventFrame, EventsResume } from '../types-system';

function removed(id: string): AlfaEvent {
  return { type: 'SessionRemoved', session_id: id };
}

function frame(prev: number, seq: number, id: string, epoch = 'e1'): EventFrame {
  return { v: 1, epoch, prev, seq, events: [removed(id)] };
}

interface Rig {
  readonly sequencer: EventSequencer;
  readonly seen: string[];
  readonly asked: number[];
  lost: number;
  answer(result: EventsResume | Error): Promise<void>;
}

function rig(): Rig {
  const seen: string[] = [];
  const asked: number[] = [];
  let settle: ((result: EventsResume | Error) => void) | null = null;
  const out: Rig = {
    seen,
    asked,
    lost: 0,
    sequencer: new EventSequencer({
      deliver: (events) => {
        for (const e of events) if (e.type === 'SessionRemoved') seen.push(e.session_id);
      },
      resume: (afterSeq) => {
        asked.push(afterSeq);
        return new Promise<EventsResume>((resolve, reject) => {
          settle = (result) => (result instanceof Error ? reject(result) : resolve(result));
        });
      },
      lost: () => {
        out.lost += 1;
      },
    }),
    async answer(result) {
      settle?.(result);
      settle = null;
      await Promise.resolve();
      await Promise.resolve();
    },
  };
  return out;
}

describe('kolejność ramek zdarzeń', () => {
  it('ramki po kolei trafiają dalej, powtórzone są pomijane', () => {
    const r = rig();
    // Pierwsza ramka po starcie okna może mieć dowolne `prev`.
    r.sequencer.push(frame(40, 42, 'a'));
    r.sequencer.push(frame(42, 45, 'b'));
    r.sequencer.push(frame(42, 45, 'b'));
    r.sequencer.push(frame(40, 42, 'a'));
    expect(r.seen).toEqual(['a', 'b']);
    expect(r.asked).toEqual([]);
    expect(r.sequencer.position).toBe(45);
  });

  it('luka: brakujące ramki przychodzą z rdzenia, a nowe czekają na swoją kolej', async () => {
    const r = rig();
    r.sequencer.push(frame(0, 1, 'a'));
    r.sequencer.push(frame(3, 4, 'd'));
    r.sequencer.push(frame(4, 5, 'e'));
    expect(r.seen).toEqual(['a']);
    expect(r.asked).toEqual([1]);
    await r.answer({
      epoch: 'e1',
      latest_seq: 4,
      complete: true,
      frames: [frame(1, 2, 'b'), frame(2, 3, 'c'), frame(3, 4, 'd')],
    });
    expect(r.seen).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect(r.lost).toBe(0);
    expect(r.sequencer.position).toBe(5);
  });

  it('ramek nie ma już w buforze: sygnał „stan od nowa”, potem zwykła praca', async () => {
    const r = rig();
    r.sequencer.push(frame(0, 1, 'a'));
    r.sequencer.push(frame(8, 9, 'x'));
    await r.answer({ epoch: 'e1', latest_seq: 9, complete: false, frames: [] });
    expect(r.lost).toBe(1);
    // Ramka 9 jest już w stanie, który klient wczyta; następna idzie normalnie.
    expect(r.seen).toEqual(['a']);
    r.sequencer.push(frame(9, 10, 'y'));
    expect(r.seen).toEqual(['a', 'y']);
    expect(r.asked).toEqual([1]);
  });

  it('ramka nowsza niż odpowiedź o utracie nie wywołuje drugiego pytania', async () => {
    const r = rig();
    r.sequencer.push(frame(0, 1, 'a'));
    r.sequencer.push(frame(8, 9, 'x'));
    r.sequencer.push(frame(9, 12, 'z'));
    await r.answer({ epoch: 'e1', latest_seq: 9, complete: false, frames: [] });
    expect(r.lost).toBe(1);
    expect(r.seen).toEqual(['a', 'z']);
    expect(r.asked).toEqual([1]);
  });

  it('rdzeń nie odpowiada na wznowienie: stan od nowa, wstrzymane ramki idą dalej', async () => {
    const r = rig();
    r.sequencer.push(frame(0, 1, 'a'));
    r.sequencer.push(frame(5, 6, 'f'));
    await r.answer(new Error('brak łącza'));
    expect(r.lost).toBe(1);
    expect(r.seen).toEqual(['a', 'f']);
    r.sequencer.push(frame(6, 7, 'g'));
    expect(r.seen).toEqual(['a', 'f', 'g']);
  });

  it('inne uruchomienie rdzenia: stan od nowa i numeracja od początku', () => {
    const r = rig();
    r.sequencer.push(frame(0, 50, 'a'));
    r.sequencer.push(frame(0, 2, 'b', 'e2'));
    r.sequencer.push(frame(2, 3, 'c', 'e2'));
    expect(r.lost).toBe(1);
    expect(r.seen).toEqual(['a', 'b', 'c']);
    expect(r.asked).toEqual([]);
  });
});
