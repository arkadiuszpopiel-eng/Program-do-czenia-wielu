// Kolejność ramek zdarzeń po stronie klienta (plan v6, zadanie B.2; ADR 0019). Rdzeń numeruje
// zdarzenia; ramka niesie `seq` (numer ostatniego zdarzenia) i `prev` (`seq` ramki poprzedniej).
// Gdy `prev` nie zgadza się z ostatnią ramką, którą klient dostał, po drodze coś zginęło — klient
// prosi o brakujące ramki (`events_resume`), a nowe trzyma do czasu odpowiedzi. Jeżeli rdzeń już
// ich nie ma (albo to inne uruchomienie rdzenia), klient dostaje sygnał „stan do wczytania od nowa”.
import type { AlfaEvent, EventFrame, EventsResume } from './types-system';

export interface SequencerHooks {
  /** Zdarzenia w kolejności, bez luk i bez powtórzeń. */
  deliver(events: readonly AlfaEvent[]): void;
  /** Prośba o ramki po `afterSeq`. */
  resume(afterSeq: number, epoch: string): Promise<EventsResume>;
  /** Zdarzeń nie da się odtworzyć — klient ma wczytać stan od nowa. */
  lost(): void;
}

export class EventSequencer {
  private epoch: string | null = null;
  private last = 0;
  /** Ramki, które przyszły w czasie odzyskiwania (`null` — nic nie odzyskujemy). */
  private held: EventFrame[] | null = null;

  constructor(private readonly hooks: SequencerHooks) {}

  /** `seq` ostatniej przyjętej ramki (0 — jeszcze żadnej). */
  get position(): number {
    return this.last;
  }

  push(frame: EventFrame): void {
    if (this.held) {
      this.held.push(frame);
      return;
    }
    if (this.epoch === null) {
      // Pierwsza ramka po starcie okna: stan i tak jest właśnie wczytywany.
      this.epoch = frame.epoch;
      this.accept(frame);
      return;
    }
    if (frame.epoch !== this.epoch) {
      // Rdzeń uruchomił się ponownie: stare numery nic nie znaczą.
      this.epoch = frame.epoch;
      this.last = 0;
      this.hooks.lost();
      this.accept(frame);
      return;
    }
    if (frame.seq <= this.last) return;
    if (frame.prev === this.last) {
      this.accept(frame);
      return;
    }
    this.recover(frame, this.epoch);
  }

  private accept(frame: EventFrame): void {
    this.last = frame.seq;
    if (frame.events.length > 0) this.hooks.deliver(frame.events);
  }

  private recover(frame: EventFrame, epoch: string): void {
    this.held = [frame];
    const finish = (): void => {
      const held = this.held ?? [];
      this.held = null;
      for (const next of held) this.push(next);
    };
    this.hooks.resume(this.last, epoch).then(
      (result) => {
        if (result.complete && result.epoch === epoch) {
          for (const missing of result.frames) if (missing.seq > this.last) this.accept(missing);
        } else {
          this.epoch = result.epoch;
          this.last = Math.max(this.last, result.latest_seq);
          this.hooks.lost();
          this.skipTo(result.latest_seq);
        }
        finish();
      },
      () => {
        // Rdzeń nie odpowiedział: nie wiemy, co zginęło — stan od nowa, a wstrzymane ramki dalej.
        this.hooks.lost();
        this.skipTo(frame.seq - 1);
        finish();
      },
    );
  }

  /** Po utracie: wstrzymane ramki są nowsze od stanu, który klient zaraz wczyta — nie żądamy luki przed nimi. */
  private skipTo(seq: number): void {
    const first = this.held?.find((f) => f.seq > seq);
    this.last = first ? first.prev : Math.max(this.last, seq);
  }
}
