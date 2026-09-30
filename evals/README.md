# evals/

Zadania wzorcowe, zamrożone zestawy akceptacyjne fal, korpus własny i benchmarki (docs/PLAN.md §3.6, §4.4;
progi w `docs/ACCEPTANCE.md`). Struktura:

| Katalog | Zawartość | W git? |
|---|---|---|
| `acceptance/` | zamrożone zestawy akceptacyjne per fala (`F0/`, `F1/`…), plik `HASHES` z sumami SHA-256 | tak |
| `corpus/` | korpus własny: nagrania głosu, transkrypcje, dane osobiste | **nie** (`.gitignore`) |
| `holdout/` | zestawy trzymane poza zasięgiem Ulepszacza (ochrona przed Goodhartem, PLAN §12.4) | tak, hash w `acceptance/HASHES` |
| `bench/` | benchmarki głosu i opóźnień (wyniki jako artefakty CI, nie w repo) | tylko definicje |

Zasady:
- Zestawy są **zamrożone hashem**: zmiana pliku bez aktualizacji `HASHES` = czerwone CI; zmiana progów
  i zestawów wymaga zatwierdzenia człowieka (AGENTS.md „Czego nie wolno”).
- Ulepszacz (`improver`) ma dostęp tylko do `acceptance/`, nigdy do `holdout/` ani `corpus/`.
- Testy ewaluacyjne uruchamia moduł `evals` (F4+); w F0 katalog jest szkieletem.
