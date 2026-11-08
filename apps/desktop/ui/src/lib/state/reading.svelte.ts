// Czytane właśnie zdanie (przycisk „Przeczytaj na głos”, plan 43). Ustawia je zdarzenie
// `SpeechSegment` z rdzenia w chwili, gdy zdanie zaczyna brzmieć; czyści koniec czytania
// (`index == of`) albo cisza po „Stop” (`SpeechLevel` bez agentki). Czyta je wiadomość
// (wyróżnienie w treści) i jej awatar (litery z ust pochodzą z czytanego zdania).
import type { Turn } from '../api/types';

export interface SpeechSegmentEvent {
  readonly turn_id: string;
  readonly index: number;
  readonly of: number;
  readonly text: string;
}

class ReadingState {
  /** Czytana tura (`null` — nic nie jest czytane). */
  turnId = $state<string | null>(null);
  /** Tekst mówiony bieżącego zdania. */
  text = $state('');
  /** Numer zdania (od 0) i liczba zdań. */
  index = $state(0);
  of = $state(0);

  apply(event: SpeechSegmentEvent): void {
    if (event.index >= event.of) {
      if (this.turnId === event.turn_id) this.clear();
      return;
    }
    this.turnId = event.turn_id;
    this.text = event.text;
    this.index = event.index;
    this.of = event.of;
  }

  clear(): void {
    this.turnId = null;
    this.text = '';
    this.index = 0;
    this.of = 0;
  }
}

export const reading = new ReadingState();

/** Awatar wiadomości: mówi przy pisaniu odpowiedzi i przy czytaniu jej na głos. */
export function voiceOf(
  turn: Turn,
  streaming: boolean,
): { speaking: boolean; words: string | undefined } {
  const read = reading.turnId === turn.id;
  return {
    speaking: streaming || read,
    words: streaming ? turn.text : read ? reading.text : undefined,
  };
}
