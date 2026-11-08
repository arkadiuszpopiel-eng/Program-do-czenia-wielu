# app-logs

Dziennik diagnostyczny procesów Alfy (kategoria `app-*`; PLAN §13, SPEC `core-log` — sekcja „Fala 5”).

- Subskrybent `tracing` instalowany w `main` każdego procesu: powłoka Tauri (`alfa-desktop` → pliki `alfa.*`),
  `alfa-broker`, `alfa-broker-ui`, `alfa-watchdog` (`app-safety::start_logs`). Bez niego wszystkie `tracing::…`
  przepadały (także `latency_us` kill-switcha).
- Plik `%LOCALAPPDATA%\Alfa\logs\<proces>.<RRRR-MM-DD>.<NNN>.log` (czas UTC), jedno zdarzenie = jedna linia
  `czas POZIOM cel: komunikat pole=wartość`; rotacja po dniu i po 10 MiB, najwyżej 14 plików na proces, retencja
  7 dni (`[logs] file_days`, 1–90). Pliki `.log` nie trafiają do eksportu `.alfa` (kategoria „Logi” bierze `*.ndjson`).
- Poziom: `ALFA_LOG` (pierwszeństwo; składnia `info`, `debug`, `warn,app_core=debug`), potem `[logs] level`
  z konfiguracji (po zbudowaniu rdzenia), domyślnie `info`. Biblioteki spoza Alfy (`hyper`, `h2`, `reqwest`, `wry`…)
  najwyżej `warn`, chyba że wskazane jawnie. W buildzie debug powłoka pisze też na stderr; procesy Jądra — nie
  (ich stderr to kanał do aplikacji, który `app-broker` dopisuje do dziennika aplikacji).
- **Bez sekretów i treści:** pola o nazwach sekretów (`api_key`, `token`, `password`, `klucz`, `hasło`…) →
  `[REDACTED]`; pola treści (`text`, `content`, `prompt`, `transcript`, `image`, `clipboard`…) → `[pominięto: N znaków]`;
  reszta przez `core_log_contract::RegexRedactor` + wzorce dodatkowe (GitHub PAT, AWS, JWT, PEM, `hf_`, `gsk_`,
  `pplx-`) + nieprzezroczyste tokeny ≥ 32 znaki; obcięcie (komunikat 4096, pole 1024 znaki, ≤ 32 pola) i ucieczka
  znaków sterujących oraz znaków kierunku tekstu (nie da się sfałszować linii).
- Panika (przed `abort` w release) zapisywana jako `ERROR alfa_panic: panika: …`.
- Bez nowych crate'ów: własny `Subscriber` na `tracing-core` (`tracing-subscriber`/`tracing-appender` nie ma
  w `Cargo.lock`).

Testy: `tests/secrets.rs` (test szpiegowski: klucz w polu, w komunikacie, w `Debug` struktury, w błędzie, token bez
prefiksu, treść rozmowy — nic nie trafia do pliku; jedna linia na zdarzenie; poziomy i zmiana w czasie działania;
`ALFA_LOG` ma pierwszeństwo), `tests/rotation.rs` (rozmiar, dzień, limit liczby, retencja 7 dni i z konfiguracji,
kontynuacja po restarcie, obce pliki nietknięte), `tests/install.rs` (instalacja globalna i panika).
