# evals/acceptance/

Zamrożone zestawy akceptacyjne per fala. Każda fala ma katalog `F<n>/` z zadaniami (JSON/TOML/WAV-ref)
i progami odwołującymi się do `docs/ACCEPTANCE.md`.

## Zamrażanie zestawu (hash)

1. Umieść pliki zestawu w `evals/acceptance/F<n>/`.
2. Wygeneruj sumy (ścieżki względem tego katalogu, sortowane, LF):
   ```bash
   cd evals/acceptance
   find F* -type f | LC_ALL=C sort | xargs sha256sum > HASHES
   ```
3. Weryfikacja (lokalnie i w CI):
   ```bash
   cd evals/acceptance && sha256sum --check --strict HASHES
   ```
4. Commit `HASHES` razem z zestawem. Zmiana zestawu bez nowego `HASHES` = czerwone CI;
   zmiana `HASHES` wymaga przeglądu człowieka (AGENTS.md).

Plik `HASHES` na starcie jest pusty (brak zestawów w F0); `sha256sum --check` na pustym pliku
zwraca błąd „no properly formatted checksum lines”, dlatego CI pomija sprawdzenie, gdy plik ma 0 linii.
