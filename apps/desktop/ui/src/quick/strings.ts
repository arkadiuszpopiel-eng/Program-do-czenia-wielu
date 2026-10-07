// Teksty okna Szybkiego pytania (osobny, mały słownik — okno ma własny budżet ≤ 40 KB gzip).
import type { Locale } from '../lib/api/types';

const STRINGS = {
  pl: {
    title: 'Szybkie pytanie',
    placeholder: 'Zapytaj Alfę…',
    label: 'Pytanie',
    answer: 'Odpowiedź',
    hintAsk: 'Enter — zapytaj · Esc — zamknij',
    hintExpand: 'Enter — otwórz w pełnym oknie · Esc — zamknij',
    open: 'Otwórz w pełnym oknie',
    thinking: 'Myśli…',
    error: 'Nie udało się: {message}',
    queued: 'Brak połączenia — pytanie czeka w kolejce.',
  },
  en: {
    title: 'Quick ask',
    placeholder: 'Ask Alfa…',
    label: 'Question',
    answer: 'Answer',
    hintAsk: 'Enter — ask · Esc — close',
    hintExpand: 'Enter — open in full window · Esc — close',
    open: 'Open in full window',
    thinking: 'Thinking…',
    error: 'Failed: {message}',
    queued: 'Offline — the question is queued.',
  },
} as const satisfies Record<Locale, Record<string, string>>;

export type QuickKey = keyof (typeof STRINGS)['pl'];

export function quickText(locale: Locale, key: QuickKey, message = ''): string {
  return STRINGS[locale][key].replace('{message}', message);
}
