# app-health

„Zdrowie systemu" w aplikacji (kategoria `app-*`).

- Diagnosta (`diagnostician-impl`): sygnały z magistrali (rejestr modułów, watchdog, `config.invalid`,
  `diagnostics.symptom` — m.in. błędy HTTP dostawców z `app-core`), incydenty, propozycje napraw
  z diffem i planem cofnięcia, „Cofnij"; naprawy Jądra wyłącznie przez Brokera. Polityka z klucza
  `diagnostician.autonomy` (od następnego uruchomienia). Porty bez implementacji produkcyjnej
  (historia rewizji konfiguracji, archiwum, pobieranie) zwracają błąd → krok cofany.
- Ulepszacz (`improver-impl`): propozycje modelu lokalnego (`LocalProposer`, Router, `LocalOnly`),
  zatwierdzenie dokładnie tego diffu (`digest`, tylko R0 — R1/R2 wymagają podpisu, brak weryfikatora
  TPM), bramka evali fail-closed (`NoReplayRunner` — bez wyników replay nic nie przechodzi);
  cykl w bezczynności tylko z monitorem sygnałów (`spawn_idle_cycle`), inaczej „Przeanalizuj teraz".
- Evale (`evals-impl`): katalog zamrożonych zestawów (integralność hashem), `HoldoutGate`
  w `%LOCALAPPDATA%\Alfa\evals\holdout` (tylko wynik zbiorczy).
- `HealthApp` — komendy `health_*`, `improver_*`, `evals_*`; most zdarzeń → `HealthChanged`.

Testy: `tests/health.rs` (symptom 401 → incydent → naprawa → „Cofnij"; odrzucenie; Ulepszacz bez
modelu nic nie wdraża), `crates/app-core/tests/{computer,signals}.rs`.
