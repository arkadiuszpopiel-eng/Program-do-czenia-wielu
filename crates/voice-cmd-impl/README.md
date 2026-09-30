# voice-cmd-impl

Szybka ścieżka komend głosowych bez LLM (`GrammarRecognizer`). Gramatyka PL/EN z `voice-cmd-contract`
jest kompilowana do wzorców (alternatywy, słowa opcjonalne, slot `{persona}` z odmianą imion).
Wypowiedź jest komendą tylko, gdy składa się wyłącznie z fraz komend, wypełniaczy („proszę”, „dobra”,
„yyy”…) i imion person — każde inne słowo oznacza zwykłą wypowiedź (→ LLM). Tolerancja szumu ASR:
porównania bez polskich znaków i odległość edycyjna (1 edycja dla słów ≥ 5 znaków, 2 dla ≥ 8).
Reguła „nie” (samodzielne, pauza ≥ 300 ms przed i po, tylko `Speaking`) jest wspólna z atrapą.
Bez adresata (PTT/wake/imię) działa tylko przerwanie mowy (stop/czekaj/anuluj/stop wszystko w `Speaking`
lub `Thinking`). Transkrypt częściowy daje trafienie po `settle_ms` (80 ms) ciszy po ostatnim słowie.

Zamrożony zestaw: `tests/data/positive.tsv` (113) i `negative.tsv` (88), sumy w `tests/data/SHA256SUMS`
(`cd tests/data && sha256sum --check SHA256SUMS`) i odcisk FNV-1a w teście — zmiana wymaga przeglądu człowieka.
Progi: recall ≥ 99 %, odrzucenie negatywów ≥ 95 %.
