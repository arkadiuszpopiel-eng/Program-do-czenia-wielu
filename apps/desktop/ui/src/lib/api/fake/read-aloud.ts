// Atrapa „Przeczytaj na głos” (plan 43): jak rdzeń — tekst mówiony (bez znaczników, kod zastąpiony
// opisem) dzielony na zdania; każde zdanie to `SpeechSegment` w chwili, gdy „zaczyna brzmieć”,
// potem poziomy mowy w rytmie sylab; na końcu `index == of` i cisza. „Stop” przerywa i wysyła ciszę
// bez agentki (jak `SpeechMeter::silence`).
import type { FakeCore } from './core';

/** Czas „mówienia” jednego znaku (ms) — krótki, żeby testy nie czekały. */
const MS_PER_CHAR = 14;
/** Odstęp poziomów mowy (jak miernik rdzenia: ok. 30 na sekundę). */
const STEP_MS = 34;

const timers = new WeakMap<FakeCore, number>();

/** Tekst mówiony: jak `lib_markdown::to_spoken_text` w uproszczeniu. */
export function spokenText(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?```/g, ' (kod na ekranie) ')
    .replace(/`([^`]*)`/g, '$1')
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}(#{1,6}|>|[-*+]|\d+\.)\s+/gm, '')
    .replace(/[*_~]+/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * Zdania jak `split_sentences` w rdzeniu: po słowach (kropka w „raport.docx” nie kończy zdania),
 * koniec po słowie zakończonym . ! ? … (bez „3. maja”).
 */
export function sentencesOf(text: string): string[] {
  const out: string[] = [];
  let current: string[] = [];
  const words = text.split(/\s+/).filter(Boolean);
  words.forEach((word, i) => {
    current.push(word);
    const last = word.at(-1) ?? '';
    const ordinal = /^\d+\.$/.test(word) && /^\p{Ll}/u.test(words[i + 1] ?? '');
    if ('!?…'.includes(last) || (last === '.' && !ordinal)) {
      out.push(current.join(' '));
      current = [];
    }
  });
  if (current.length > 0) out.push(current.join(' '));
  return out;
}

/** Przerywa czytanie (Esc, „Zatrzymaj”, nowe czytanie). */
export function stopFakeReading(core: FakeCore, silence = true): void {
  const handle = timers.get(core);
  if (handle === undefined) return;
  core.scheduler.clearTimeout(handle);
  timers.delete(core);
  if (silence) core.emit([{ type: 'SpeechLevel', agent: '', level: 0 }]);
}

export function fakeReadAloud(core: FakeCore, turnId: string): void {
  stopFakeReading(core, false);
  const turn = core.findTurn(turnId);
  if (!turn || turn.author === 'user') return;
  const agent = turn.author;
  const sentences = sentencesOf(spokenText(turn.text));
  const of = sentences.length;
  const next = (index: number, step: number, steps: number): void => {
    if (index >= of) {
      timers.delete(core);
      core.emit([
        { type: 'SpeechSegment', turn_id: turnId, index: of, of, text: '' },
        { type: 'SpeechLevel', agent, level: 0 },
      ]);
      return;
    }
    if (step === 0) {
      const text = sentences[index] ?? '';
      core.emit([{ type: 'SpeechSegment', turn_id: turnId, index, of, text }]);
      steps = Math.max(3, Math.round((text.length * MS_PER_CHAR) / STEP_MS));
    }
    const level = Math.abs(Math.sin(step / 2.3)) * 0.7 + 0.1;
    core.emit([{ type: 'SpeechLevel', agent, level }]);
    const done = step + 1 >= steps;
    timers.set(
      core,
      core.scheduler.setTimeout(
        () => next(done ? index + 1 : index, done ? 0 : step + 1, steps),
        STEP_MS,
      ),
    );
  };
  next(0, 0, 0);
}
