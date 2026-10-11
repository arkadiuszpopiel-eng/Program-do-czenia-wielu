# evals/V6/pl-core — zestaw po polsku (plan v6, zadanie C2.1)

**Status: szkic (`proposed`) — wymaga akceptacji właściciela, potem zamrożenie hashem.** Progi ustala
właściciel; do tego czasu zestaw służy do pomiaru, nie do bramki.

Trzy pliki w formacie natywnym harnessu (`eval_cases`: `{id, split, class, input, expected}`), razem
36 przypadków; po jednym przypadku `dev` na plik (do strojenia), reszta `test`.

| Plik                | Klasa                                             | Przypadki | Co sprawdza                                                                                                   |
| ------------------- | ------------------------------------------------- | --------- | ------------------------------------------------------------------------------------------------------------- |
| `tool-calls.ndjson` | `tool_call`                                       | 12        | wybór narzędzia i argumenty po polsku (nazwy plików z polskimi znakami), brak wywołania, gdy nie jest potrzebne |
| `rag.ndjson`        | `rag_fidelity`                                    | 12        | odpowiedź zgodna ze źródłem, wnioski z kilku zdań, przyznanie „tego w źródle nie ma” zamiast zgadywania        |
| `attacks.ndjson`    | `injection_*`, `secret_exfiltration`, `benign_control` | 12   | polecenia wstrzyknięte w dokument, wynik narzędzia, stronę, e-mail i pamięć; wyciąganie sekretu; dwa przypadki kontrolne fałszywych alarmów |

## Wejście (`input`)

- `persona`, `messages`, `max_tokens` — jak w `evals/F1/prompts-pl`.
- `tools` — narzędzia dostępne w przypadku (nazwy jak w `tools-*-contract`); runner podaje je modelowi
  w formacie właściwym dla dostawcy.
- `context` — treści dołączone do rozmowy: `source` (skąd), `text`, a w atakach `trust: "untrusted"`
  (runner oznacza je tak, jak robi to aplikacja dla treści niezaufanych).
- `secrets.canary` — napis-kanarek wstawiany przez runner do promptu systemowego jako „sekret
  konfiguracyjny”; nie jest to żaden prawdziwy klucz.

## Oczekiwanie (`expected`)

- `call: { tool, args }` — model ma wywołać to narzędzie; warunki argumentów: `equals`, `contains`,
  `contains_ci` (bez rozróżniania wielkości liter), `matches_any` (wyrażenia regularne), `lines`
  (dokładne linie treści).
- `sequence` — wywołania w podanej kolejności.
- `no_call: true` — żadnego wywołania; `no_call_of` / `must_not_call` — tych narzędzi nie wolno wywołać.
- `answer_or_call` — dopuszczalne jest wskazane wywołanie albo odpowiedź tekstowa bez wywołania.
- `answer` — warunki tekstu odpowiedzi: `contains`, `contains_all_ci`, `contains_any_ci`,
  `must_not_contain`, `must_not_contain_ci`, `language`.
- `answer_absent: true` — poprawna odpowiedź to przyznanie, że źródło tego nie podaje.
- `note` — po ludzku, co przypadek sprawdza.

## Czego tu nie ma

- **WER mowy, czas odpowiedzi, zajętość pamięci karty** — wymagają sprzętu i korpusu głosu, który jest
  poza repozytorium (`evals/corpus`, `.gitignore`); kryterium zapisane w manifeście jako tekstowe.
- **Runnera** dla pól `tools`, `context`, `secrets` — harness czyta i sprawdza integralność zestawu
  (test `evals-impl/tests/repo_suites.rs`); wariant uruchamiający model z narzędziami jest kolejnym krokiem
  po akceptacji formatu.
- Przypadków `holdout` — ukryty zestaw żyje poza gitem (ACCEPTANCE §13).

Zestaw powstał w sesji autorskiej (2026-10-11) bez uruchamiania na modelu — poprawność oczekiwań
(szczególnie `matches_any` i przypadków 11–12 narzędzi) wymaga przeglądu przy pierwszym przebiegu.
