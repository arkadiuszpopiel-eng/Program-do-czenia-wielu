// Skrypty odpowiedzi atrapy — deterministyczne (zależne tylko od treści i numeru wariantu).
import type { AgentId } from '@alfa/ui-kit';
import type { ApprovalPending, ToolIcon } from '../types';
import {
  CANVAS_ARTIFACTS_REPLY,
  CANVAS_DOCUMENT_REPLY,
  CANVAS_REVISION_REPLY,
  canvasEdgeTokens,
} from './canvas-replies';

export interface ScriptTool {
  readonly icon: ToolIcon;
  readonly label: string;
  readonly ms: number;
  readonly undo: boolean;
  /** Narzędzie (nazwa z manifestu) — krok w Replay. */
  readonly tool?: string;
  /** „Uruchom w terminalu": polecenie do skopiowania (bez wykonania). */
  readonly terminal?: string;
}

export interface ResponseScript {
  readonly agent: AgentId;
  readonly role_id: string;
  readonly text: string;
  readonly thinkingMs: number;
  readonly tools: readonly ScriptTool[];
  readonly approval: Omit<ApprovalPending, 'id' | 'status' | 'broker_window' | 'expires_at'> | null;
  /** Odpowiedź ucięta (StopReason MaxTokens) → akcja „Kontynuuj". */
  readonly truncated: boolean;
  /** Własny podział tekstu na porcje strumienia (np. bardzo długi artefakt w kilku porcjach). */
  readonly tokens?: readonly string[];
  /** Model w `Usage` (Arena); brak — domyślny model atrapy. */
  readonly model?: { readonly provider: string; readonly model: string };
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
  if (has(input, 'kanw') && has(input, 'graniczn')) {
    const tokens = canvasEdgeTokens();
    return { ...base(addressed ?? 'gama', 'coder', tokens.join('')), tokens };
  }
  if (has(input, 'kanw') && has(input, 'popraw')) {
    return base(addressed ?? 'gama', 'coder', CANVAS_REVISION_REPLY);
  }
  if (has(input, 'kanw')) {
    const doc = has(input, 'dokument');
    const text = doc ? CANVAS_DOCUMENT_REPLY : CANVAS_ARTIFACTS_REPLY;
    return base(addressed ?? 'gama', doc ? 'researcher' : 'coder', text);
  }
  if (has(input, 'kod', 'code', 'funkcj', 'skrypt')) {
    return {
      ...base(
        addressed ?? 'delta',
        'coder',
        v === 0
          ? 'Przygotowałam funkcję i krótki test. Mogę ją uruchomić w terminalu, jeśli chcesz.\n\n```ts\nexport function sumaNetto(pozycje: { cena: number; ilosc: number }[]): number {\n  return pozycje.reduce((suma, p) => suma + p.cena * p.ilosc, 0);\n}\n```\n\nDaj znać, czy dodać obsługę rabatów.'
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
        {
          icon: 'search',
          label: 'Przeszukano Pobrane (31 plików)',
          ms: 500,
          undo: false,
          tool: 'fs_search',
        },
        {
          icon: 'edit',
          label: 'Delta: przeniesiono 14 plików',
          ms: 1200,
          undo: true,
          tool: 'fs_move',
        },
        {
          icon: 'terminal',
          label: 'Uruchom w terminalu',
          ms: 300,
          undo: false,
          tool: 'shell_terminal',
          terminal: 'Get-ChildItem -Recurse Archiwum/2026 | Measure-Object',
        },
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
