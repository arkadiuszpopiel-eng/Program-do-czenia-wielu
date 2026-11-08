# evals/F1/prompts-pl — 20 promptów PL dla lokalnego llama.cpp (ACCEPTANCE F1-03)

**Status: szkic — wymaga akceptacji człowieka, potem zamrożenie hashem.** Autorka: model-recenzentka (fala 4).

Kryterium **F1-03**: lokalny `llama-server` (llama.cpp, model 3–4,5B — domyślnie Bielik 4.5B v3.0 Instruct
z menedżera modeli; oficjalne GGUF Bielika ma tylko `Q8_0`, nie `Q4_K_M` z ACCEPTANCE — ADR 0014) odpowiada na 20 promptów PL **20/20 bez błędu**, a szybkość generowania
**tok/s ≥ wartość z F0-13** (`evals/spikes/h-sprzet`, `bench-llama.ps1`, `tg128`). Maszyny: desktop z emulacją
baseline i laptop (CUDA). Weryfikuje CI na runnerze self-hosted.

## Zawartość

`prompts.ndjson` — 20 przypadków w formacie natywnym harnessu (`eval_cases`: `{id, split, class, input,
expected}`), wszystkie w podziale `test`.

| Klasa          | Przypadki      | Co sprawdza                                                              |
| -------------- | -------------- | ------------------------------------------------------------------------ |
| `wiedza`       | 01             | krótka odpowiedź faktograficzna                                          |
| `rozumowanie`  | 02, 11, 12, 16 | arytmetyka słowna, daty (dzień tygodnia), jednostki, liczby w zapisie PL |
| `tekst`        | 03, 04, 13, 14 | streszczenie, e-mail, wiersz, instrukcja w punktach                      |
| `tlumaczenie`  | 05, 06         | PL → EN (odpowiedź po angielsku), EN → PL                                |
| `format`       | 07, 08         | lista numerowana, czysty JSON                                            |
| `jezyk`        | 09, 15         | odmiana przez przypadki, pytanie bez polskich znaków                     |
| `kod`          | 10             | funkcja w Pythonie z opisem po polsku                                    |
| `klasyfikacja` | 17             | wydźwięk zdania jednym słowem                                            |
| `kontekst`     | 18, 20         | rozmowa wieloturowa; dłuższe wejście (regulamin ~290 słów, prefill)      |
| `persona`      | 19             | agentka mówi w rodzaju żeńskim („sprawdziłam”, nie „sprawdziłem”)        |

## Format przypadku

- `input.persona` — persona, której prompt systemowy buduje runner tak jak aplikacja (`personas`, wbudowana
  `alfa`); `input.messages` — historia (`user`/`assistant`); `input.max_tokens` — dobrany z zapasem, żeby poprawna
  odpowiedź kończyła się `stop`.
- `expected.must` — **kryterium F1-03** („bez błędu”). Zawsze, dla każdego przypadku: brak błędu transportu i
  serwera; `finish_reason = stop` (nie `length`); odpowiedź niepusta (≥ `min_chars`); poprawny UTF-8 bez `U+FFFD`
  i znaków sterujących poza `\n`, `\r`, `\t`; **bez wyciekłych tokenów szablonu czatu** (`<|im_start|>`,
  `<|im_end|>`, `<s>`, `</s>`, `<|eot_id|>`, `<|endoftext|>`, `[INST]` — objaw złego szablonu w `llama-server`);
  bez zapętlenia (ta sama linia ≥ 20 znaków powtórzona ≥ 3 razy); język odpowiedzi `language` (heurystyka: dla
  `pl` — polski znak diakrytyczny albo ≥ 2 częste słowa polskie; dla `en` — ≥ 2 częste słowa angielskie, polskie
  znaki dozwolone w nazwach własnych; `ignore_code_blocks` — bloki kodu pomijane).
- `expected.quality` — **jakość treści, raportowana, nieblokująca** (do decyzji człowieka, czy włączyć do progu):
  `contains_any_ci` (z każdej grupy co najmniej jeden fragment, bez wielkości liter), `numbers` (liczba w
  odpowiedzi w tolerancji; zapis PL „1 234,56” = 1234.56), `regex` / `regex_absent`, `json` (pierwszy obiekt JSON
  w odpowiedzi, pola `equals_ci`/`contains_ci`), `numbered_items_min`, `list_items_min`, `min_lines`,
  `max_sentences`, `max_chars`.

## Przebieg (runner — do zbudowania)

Runnera jeszcze nie ma; testy `providers-local-impl/tests/*` działają na atrapie `fake-llama-server`.
Proponowany przebieg na self-hosted (Windows, `llama-server` i model pobrane przez Alfę):

1. start sidecara przez `providers-local-impl` (te same parametry co w aplikacji: kontekst, warstwy GPU, szablon
   czatu z GGUF), rozgrzanie jednym zapytaniem spoza zestawu;
2. każdy przypadek **N = 5 razy** (ACCEPTANCE §1) z ustawieniami próbkowania aplikacji i zapisanym ziarnem,
   plus jeden przebieg deterministyczny (`temperature = 0`, `seed = 42`) do porównań między maszynami;
3. F1-03 zaliczone, gdy **100 % generacji** (20 × 5) spełnia `must`;
4. szybkość: mediana `timings.predicted_per_second` z odpowiedzi `llama-server` po generacjach z
   `predicted_n ≥ 64`, porównana z `tg128` z F0-13 dla tej samej maszyny, modelu i kwantyzacji; dodatkowo
   raportowane `prompt_per_second` (prefill — przypadek 20) i czas do pierwszego tokenu;
5. raport Markdown: wynik `must` per przypadek, jakość per klasa, tok/s, wersja llama.cpp, SHA-256 modelu.

## Do decyzji człowieka

1. Czy `quality` (lub wybrane klasy) wchodzi do progu F1-03 — ACCEPTANCE mówi tylko „bez błędu”.
2. Wartość odniesienia tok/s: F0-13 jeszcze niezmierzone (bramka #8) — do tego czasu runner raportuje bez progu.
3. Po zamrożeniu: `evals/F1/MANIFEST.json` (`schema: 1`, źródło `eval_cases`, próg
   `{"id": "F1-03", "rule": "metric", "metric": "pass_rate", "op": "ge", "value": 1.0, "point": true}`).
