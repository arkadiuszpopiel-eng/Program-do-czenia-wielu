# core-log — SPEC (szkic v0)

## Cel
Log-writer jądra: trwały zapis strumieni zdarzeń z magistrali (wywołania modeli, narzędzia i GUI, głos, diagnostyka) jako append-only NDJSON + indeks SQLite, z szyfrowaniem payloadów kluczem per sesja (crypto-shredding), retencją i limitami dysku (PLAN §13). Strumień **Audyt** docelowo pisze Broker (F3); do tego czasu `core-log` z oznaczeniem `pre-broker`.

## Fala i priorytet
F0 (kontrakt + fake), F1 (logi v1, Oś czasu v0), F3 (przekazanie Audytu Brokerowi). P0.

## Kontrakt (szkic Rust)
```rust
// core-log-contract — SZKIC
pub enum Stream { Audit, ModelCalls, ToolsGui, Voice, Diagnostics }
pub struct LogRecord { pub env: Envelope /* core-bus */, pub stream: Stream,
                       pub payload: EncryptedPayload, pub hash_prev: Option<Hash>, pub schema_version: u16 }
pub trait LogWriter: Send + Sync {
    fn append(&self, stream: Stream, ev: &Envelope) -> Result<PayloadRef, LogError>;
    fn query(&self, q: LogQuery) -> Result<Vec<LogRecord>, LogError>;   // po sesji/przebiegu/rodzaju/czasie
    fn shred_session(&self, session: SessionId) -> Result<(), LogError>;  // kasuje klucz payloadów
    fn diagnostics_bundle(&self, opts: BundleOpts) -> Result<PathBuf, LogError>;
}
pub struct Retention { pub tools_gui_days: u16 /* 7 */, pub disk_limit_mb: u32, pub redact_prompts: bool }
```
Zdarzenia: `log.rotated`, `log.disk_limit_reached`, `log.upcast_applied`, `log.audit_handover` (Broker przejmuje Audyt).

## Zależności
`core-bus-contract`, `core-config-contract`, `platform-windows-contract` (ścieżki, ACL). Baza: SQLite (ADR 8).

## Niezmienniki
- Append-only: brak API modyfikującego/usuwającego rekordy; usuwanie danych = crypto-shredding klucza sesji (łańcuch hashy pozostaje spójny).
- Wersjonowane schematy zdarzeń + upcastery; stary rekord czytany przez łańcuch upcasterów; testy migracji na fixture'ach każdej wersji.
- Redakcja sekretów (klucze, tokeny, pola `IsPassword`) przed zapisem — zawsze, niezależnie od `redact_prompts`.
- Deny-lista aplikacji/okien/URL wyklucza zrzuty i migawki UIA ze strumienia ToolsGui.
- Zapis nie blokuje wydawcy (kolejka + wątek zapisu); przy pełnym dysku: degradacja (najpierw ToolsGui, potem Voice), Audyt nigdy nie jest pomijany po cichu — błąd zapisu Audytu = zdarzenie Diagnostics + baner.

## Zdolności / uprawnienia
`fs.write(%LOCALAPPDATA%\Alfa\logs\**)` dla jądra. Po F3 pliki Audytu są zapisywalne tylko przez konto Brokera (ACL).

## Izolacja
`inproc`, `always`; osobny wątek I/O.

## Budżet zasobów
RAM ≤ 8 MB (bufory); zapis ≤ 1 ms p95 na rekord (bez fsync per rekord, grupowanie); limit dysku domyślnie 2 GB (do potwierdzenia w F1).

## Konfiguracja (klucze TOML)
`[logs] level = "info"`, `disk_limit_mb = 2048`, `retention.tools_gui_days = 7`, `redact_prompts = true`, `[logs.denylist] apps = [...], urls = [...]`.

## Wkład do UI
Dane dla Osi czasu/Replay, pulpitu kosztów/opóźnień, ekranu „co poszło do chmury", paczki diagnostycznej; strona Ustawienia → Logi i prywatność.

## Testy akceptacyjne
- `ACC-F0-core-log-01`: test kontraktowy `append/query/shred` na `-fake` i `-impl`.
- `ACC-F1-core-log-02`: upcaster v1→v2 na zamrożonych fixture'ach; 0 utraty pól.
- `ACC-F1-core-log-03`: po `shred_session` payloady nieczytelne, łańcuch hashy weryfikowalny.
- `ACC-F1-core-log-04`: redakcja — 0 sekretów z fixture'ów w plikach logów (test szpiegowski).

## Fake
`core-log-fake`: zapis do pamięci/tmp, deterministyczne id, bez szyfrowania (flaga), zliczanie rekordów per strumień — do asercji w testach innych modułów.

## Otwarte pytania
- Kotwiczenie głowy łańcucha hashy poza zasięgiem agentów (gdzie: Broker, TPM?) — ADR z THREAT_MODEL, do ustalenia w SPEC v1.
- Format indeksu (osobna baza `logs.db` vs baza per sesja) — do ustalenia po spike (i).

## Zmiany po implementacji (F0: `core-log-impl`, `core-log-fake`)
- **Kontrakt (dodane):** `LogQuery` ma `kind`, `since` (włącznie), `until` (wyłącznie) — `#[serde(default)]`; `LogQuery::matches`
  (wspólny filtr); `stream: None` = wszystkie strumienie; wynik zawsze rosnąco po (`event.ts`, strumień, `seq`), `limit` na końcu;
  `LogStream::ALL` i `Ord`; `contract_tests` (feature). `seq` numerowane od 0 per strumień.
- **Pliki:** `<root>/<strumień>/<pierwszy seq, 20 cyfr>.ndjson`, linia `{seq, schema_version, written_at, event}`; rotacja po
  `max_segment_bytes` (8 MiB); limit dysku per strumień (512 MiB = 2 GiB / 4) — usuwane najstarsze segmenty; retencja w dniach
  (Narzędzia/GUI 7, reszta bez) wg `written_at`; po usunięciu wszystkiego zostaje pusty segment od `next_seq` (numeracja przetrwa restart).
  Urwany ogon po awarii → nowy segment (stare dane niemodyfikowane); uszkodzone linie pomijane przy odczycie.
- **Redakcja:** `Redactor` (domyślnie `RegexRedactor`) na ładunku przed zapisem — strumienie i Audyt. Redakcja po nazwach pól
  (`password`, `api_key` jako klucz JSON) — propozycja do kontraktu `Redactor` (dziś tylko wzorce w wartościach).
- **Audyt pre-broker:** `PreBrokerAuditWriter` (plik NDJSON), rekord `{event, hash, seq, writer: "pre-broker", written_at}`
  w kanonicznym JSON (klucze posortowane, bez spacji), `event.prev_hash` = hash poprzednika, `hash` = SHA-256 rekordu bez `hash`.
  `verify_bytes/verify_file` wykrywają modyfikację (w tym dowolny bajt), usunięcie i wstawienie; ucięcie ogona — porównanie z głową
  (`verify_chain`/`verify_against`). Otwarcie naruszonego łańcucha → `AuditOpenError::Broken` (bez dopisywania).
- **Magistrala:** `spawn_bus_writer` — rodzaje wbudowane wg `LogStream::for_kind`, `audit` → łańcuch, zdarzenia modułów → Diagnostyka, `ui` pominięte.
- **Nie w F0:** indeks SQLite, szyfrowanie payloadów i `shred_session`, upcastery, deny-lista, kolejka + wątek zapisu (zapis synchroniczny
  pod zamkiem, bez fsync per rekord), `diagnostics_bundle`, zdarzenia `log.rotated`/`log.disk_limit_reached`, fsync Audytu.

## Fala 5: dziennik diagnostyczny procesów (`app-logs`)
- **Problem (raport H):** żaden proces nie instalował subskrybenta `tracing` — wszystkie `tracing::…` (błędy startu,
  ostrzeżenia modułów, `latency_us` kill-switcha) przepadały. Strumienie NDJSON `core-log` zapisują tylko zdarzenia
  magistrali.
- **Rozwiązanie:** crate `app-logs` (korzeń kompozycji) — własny `Subscriber` na `tracing-core` (bez nowych crate'ów:
  `tracing-subscriber`/`tracing-appender` nie ma w `Cargo.lock`), instalowany w `main`: powłoka Tauri (`alfa`),
  `alfa-broker`, `alfa-broker-ui`, `alfa-watchdog`. Plik tekstowy `%LOCALAPPDATA%\Alfa\logs\<proces>.<RRRR-MM-DD>.<NNN>.log`
  (UTC), linia `czas POZIOM cel: komunikat pole=wartość`; rotacja po dniu i po 10 MiB, ≤ 14 plików na proces
  (≤ 140 MiB), retencja 7 dni (PLAN §13; `[logs] file_days`, 1–90). Segmenty `*.ndjson` i pliki innych procesów
  nie są dotykane; `.log` nie wchodzi do eksportu `.alfa`.
- **Poziom:** `ALFA_LOG` (pierwszeństwo) → `[logs] level` (powłoka, po zbudowaniu rdzenia) → `info`. Składnia
  `poziom[,cel=poziom…]`, najdłuższy cel wygrywa; cele spoza Alfy najwyżej `warn` bez jawnego wpisu (biblioteki HTTP
  na `debug` logują nagłówki). Agentki nie mogą ustawić `ALFA*` ani `WEBVIEW2_*` (`env_write_denied`).
- **Redakcja (zawsze, przed zapisem):** nazwy pól sekretów → `[REDACTED]`; nazwy pól treści (tekst rozmowy,
  transkrypcja, prompt, obraz/piksele, schowek, audio) → `[pominięto: N znaków]`; wartości przez `RegexRedactor`
  (`core-log-contract`) + wzorce dodatkowe (GitHub PAT, AWS, JWT, PEM, `hf_`, `gsk_`, `pplx-`) + nieprzezroczyste
  tokeny (≥ 32 znaki, wielkie i małe litery oraz cyfry; hashe hex i UUID zostają); wartość > 64 KiB pominięta
  w całości, potem obcięcie (komunikat 4096, pole 1024 znaki, ≤ 32 pola) i ucieczka znaków sterujących/kierunku
  tekstu. Do pliku trafia więc tylko to, co już dziś niosą zdarzenia diagnostyczne — bez sekretów, treści i pikseli.
- **Procesy Jądra:** bez kopii na stderr (stderr to kanał do aplikacji — `app-broker` dopisuje jego linie do dziennika
  aplikacji i pokazuje ostatnią przy awarii); usługa Brokera pisze do `%LOCALAPPDATA%` konta usługi (katalogu danych
  Brokera nie dotyka — tworzy go `PrivateDirPort` z ACL). Powłoka w buildzie debug — także stderr.
- **Panika:** hook zapisuje `ERROR alfa_panic: panika: … miejsce=plik:linia` przed `abort` (release).
- **Testy:** `crates/app-logs/tests/secrets.rs` (test szpiegowski ACC-F1-core-log-04 dla dziennika procesu),
  `rotation.rs`, `install.rs`; jednostkowe filtra i redakcji.
- **Nie w fali 5:** ustawienie poziomu w UI (strona „Logi i prywatność” — dziś klucz w `shared.toml`), zmiana poziomu
  bez restartu z UI, paczka diagnostyczna z plikami `.log`.
