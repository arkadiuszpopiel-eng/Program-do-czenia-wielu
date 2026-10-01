// Skrypty odpowiedzi atrapy — deterministyczne (zależne tylko od treści i numeru wariantu).
import type { AgentId } from '@alfa/ui-kit';
import type { ApprovalPending, ToolIcon } from '../types';

export interface ScriptTool {
  readonly icon: ToolIcon;
  readonly label: string;
  readonly ms: number;
  readonly undo: boolean;
}

export interface ResponseScript {
  readonly agent: AgentId;
  readonly role_id: string;
  readonly text: string;
  readonly thinkingMs: number;
  readonly tools: readonly ScriptTool[];
  readonly approval: Omit<ApprovalPending, 'id' | 'status'> | null;
  /** Odpowiedź ucięta (StopReason MaxTokens) → akcja „Kontynuuj". */
  readonly truncated: boolean;
}

const base = (agent: AgentId, role_id: string, text: string): ResponseScript => ({
  agent,
  role_id,
  text,
  thinkingMs: 0,
  tools: [],
  approval: null,
  truncated: false,
});

const has = (text: string, ...words: string[]): boolean =>
  words.some((w) => text.toLowerCase().includes(w));

export function pickResponse(
  input: string,
  addressed: AgentId | null,
  variant: number,
): ResponseScript {
  const v = variant % 2;
  if (has(input, 'kod', 'code', 'funkcj', 'skrypt')) {
    return {
      ...base(
        addressed ?? 'delta',
        'coder',
        v === 0
          ? 'Przygotowałam funkcję i krótki test. Uruchomienie w terminalu przejdzie przez Brokera.\n\n```ts\nexport function sumaNetto(pozycje: { cena: number; ilosc: number }[]): number {\n  return pozycje.reduce((suma, p) => suma + p.cena * p.ilosc, 0);\n}\n```\n\nDaj znać, czy dodać obsługę rabatów.'
          : 'Oto wersja z walidacją danych wejściowych.\n\n```ts\nexport function sumaNetto(pozycje: { cena: number; ilosc: number }[]): number {\n  if (pozycje.some((p) => p.ilosc < 0)) throw new Error("ujemna ilość");\n  return pozycje.reduce((suma, p) => suma + p.cena * p.ilosc, 0);\n}\n```',
      ),
      thinkingMs: 900,
      tools: [{ icon: 'search', label: 'Przeszukano src/ (12 plików)', ms: 400, undo: false }],
    };
  }
  if (has(input, 'plik', 'folder', 'przenie', 'uporządk')) {
    return {
      ...base(
        addressed ?? 'delta',
        'operator',
        'Przeniosłam 14 plików do folderu Archiwum/2026. Każdy krok można cofnąć jednym kliknięciem.',
      ),
      tools: [
        { icon: 'search', label: 'Przeszukano Pobrane (31 plików)', ms: 500, undo: false },
        { icon: 'file', label: 'Przeniesiono 14 plików do Archiwum/2026', ms: 1200, undo: true },
      ],
      approval: {
        what: 'Usunięcie 6 duplikatów z folderu Pobrane',
        why: 'Pliki są identyczne z kopiami w Archiwum/2026.',
        reversible: true,
        risk: 'high',
      },
    };
  }
  if (has(input, 'długi', 'dlugi', 'esej', 'szczegółow')) {
    return {
      ...base(
        addressed ?? 'gama',
        'researcher',
        'Zacznę od kontekstu. W trzecim kwartale sprzedaż wzrosła, ale struktura kosztów zmieniła się bardziej, niż wynika z samych sum. Najpierw omówię przychody według segmentów, potem koszty stałe i zmienne, a na końcu ryzyka na kolejny kwartał.\n\nSegment B2B odpowiada za większość wzrostu: nowi klienci z sektora publicznego podpisali umowy roczne, co stabilizuje przychody, ale wydłuża cykl płatności',
      ),
      thinkingMs: 1500,
      truncated: true,
    };
  }
  if (has(input, 'plan', 'tydzie', 'kalendarz', 'zakupy')) {
    return base(
      addressed ?? 'beta',
      'keeper',
      v === 0
        ? 'Ułożyłam plan:\n- poniedziałek: raport dla zarządu,\n- środa: przegląd umów,\n- piątek: zakupy i podsumowanie tygodnia.\n\nZapisać go w kalendarzu?'
        : 'Proponuję lżejszy plan: dwa bloki pracy głębokiej rano i wolne popołudnie w piątek. Mam go zapisać?',
    );
  }
  return {
    ...base(
      addressed ?? 'alfa',
      'conductor',
      v === 0
        ? 'Jasne. Rozpisałam to na trzy kroki: najpierw zbiorę dane, potem przygotuję szkic, a na końcu sprawdzę spójność liczb. Gdy coś będzie wymagało Twojej zgody, dam znać.'
        : 'Mogę to zrobić od razu. Zacznę od najprostszego kroku i będę Cię informować o postępach w kapsule aktywności.',
    ),
    thinkingMs: v === 0 ? 600 : 0,
  };
}

export function quickResponse(input: string): ResponseScript {
  return base(
    'alfa',
    'conductor',
    has(input, 'pogod')
      ? 'Nie mam dostępu do prognozy bez klucza dostawcy pogody. Mogę za to otworzyć stronę z prognozą w przeglądarce.'
      : `Krótko: ${input.trim().replace(/\?+$/, '')} — sprawdziłam i przygotowałam odpowiedź. Naciśnij Enter, aby otworzyć rozmowę w pełnym oknie.`,
  );
}
