# Format paczki `.alfa` (moduł `transfer`, PLAN §15.1)

Szkic v0. Paczka `.alfa` służy do **przenoszenia danych między maszynami właściciela** (import/eksport) i do **kopii zapasowych**. Nie ma automatycznej synchronizacji (PLAN §19). Ten sam format i ten sam kod obsługują eksport ręczny, eksport pojedynczej sesji z panelu Sesje i zaplanowane kopie zapasowe.

## 1. Kontener

- **Archiwum ZIP** (deflate; duże pliki artefaktów mogą być `store`). Rozszerzenie `.alfa`; typ MIME `application/vnd.alfa.package+zip` (do ustalenia w SPEC v1 `transfer`).
- Pierwszym wpisem archiwum jest `manifest.json` (nieskompresowany) — pozwala na podgląd bez rozpakowania całości.
- Wszystkie ścieżki wewnątrz względne, `/` jako separator, bez `..`, bez ścieżek absolutnych (import odrzuca paczkę naruszającą tę regułę — ochrona przed zip-slip).
- Kodowanie tekstu: UTF-8. Daty: RFC 3339 z UTC.

## 2. Układ katalogów

```
manifest.json                       metadane, zakres, sumy kontrolne (§3)
config/
  common/*.toml                     konfiguracja wspólna (agentki, obsada, reguły, uprawnienia, biblie głosów)
  machine/<machine-id>.toml         nakładka per maszyna (domyślnie NIE eksportowana)
personas/<persona>/                 biblie głosu, słownik wymowy, referencje głosowe (jeśli dozwolone licencją)
casts/*.yaml                        szablony obsad ról
rules/*.yaml                        reguły Marszałka (P1)
skills/<id>/                        umiejętności i manifesty agentek z Kreatora (P1)
sessions/<session-id>/
  session.json                      metadane sesji (§11 PLAN)
  events.ndjson                     dziennik zdarzeń sesji (append-only, wersja schematu w każdym rekordzie)
  branches.json                     drzewo gałęzi (projekcja rozmowy)
  attachments/                      załączniki (opcjonalnie)
memory/<scope>/<id>.ndjson          wpisy pamięci z proweniencją (P1; zakresy: sesja/projekt/globalna/agentka)
artifacts/<session-id>/             pliki oddane przez agentki (opcjonalnie)
logs/                               logi (domyślnie NIE eksportowane)
secrets.enc                         TYLKO w osobnym „eksporcie sekretów" (§5) — nigdy w zwykłej paczce
```

Katalogi nieużyte w danym zakresie po prostu nie występują. Import ignoruje nieznane katalogi, ale wypisuje je w dry-run jako „pominięte".

## 3. `manifest.json`

Pola (v1 schematu):

| Pole | Znaczenie |
|---|---|
| `schema_version` | wersja schematu paczki (semver; migracje upcasterami §7) |
| `app_version` | wersja Alfy, która wykonała eksport |
| `created_at` | data eksportu (RFC 3339, UTC) |
| `source_machine` | `{ id, name, os, hw_class }` — `id` z `device-profile`, `hw_class` np. `standard-amd`, `laptop-cuda` |
| `scope` | które elementy weszły (lista kluczy zakresu, §4) i liczniki (sesji, wpisów pamięci itd.) |
| `content` | lista wpisów `{ path, sha256, bytes }` dla **każdego** pliku poza manifestem |
| `content_sha256` | skrót nad posortowaną listą `content` (szybka weryfikacja integralności całości) |
| `encryption` | `null` albo `{ scheme, kdf, salt, nonce }` (§5) |
| `kind` | `export` \| `backup` \| `secrets` |
| `notes` | opcjonalny opis od użytkownika |

Sumy kontrolne to **integralność**, nie uwierzytelnienie: paczka niezaszyfrowana nie jest podpisana (paczki są tworzone i importowane przez tego samego właściciela). Podpis minisign dla kopii zapasowych — do ustalenia w SPEC v1.

## 4. Co wchodzi, co nie wchodzi

Zakres wybiera użytkownik w kreatorze (Ustawienia → Import i eksport, paleta, głos):

| Klucz zakresu | Zawartość | Domyślnie |
|---|---|---|
| `config.common` | konfiguracja wspólna | tak |
| `personas` | agentki, biblie głosów, słownik wymowy | tak |
| `casts` | obsady ról | tak |
| `rules` | reguły (P1) | tak |
| `skills` | umiejętności / agenci z Kreatora (P1) | tak |
| `sessions[]` | wybrane sesje (id) | wybór |
| `memory[]` | wybrane zakresy pamięci (P1) | wybór |
| `artifacts` | artefakty wybranych sesji | nie |
| `logs` | logi | nie |
| `config.machine` | nakładka maszyny (urządzenia audio, profil, limity) | nie |

**Nigdy w paczce:** klucze API, tokeny, hasła, zawartość Credential Managera, poświadczenia CLI (`~/.claude`, `~/.codex`), klucz szyfrowania baz sesji w postaci jawnej. Eksporter ma test „szpiegowski": paczka z pełnym zakresem nie zawiera żadnego sekretu z fixture'ów.

Sesje z tagiem prywatności „prywatne" eksportują się tylko przy jawnym zaznaczeniu i zawsze w paczce szyfrowanej (propozycja; do ustalenia w SPEC v1).

Zakres P0-lite (F1): `config.common`, `personas`, `casts`, `sessions[]`. Reszta w F7 (PLAN §16.1).

## 5. Szyfrowanie

- **Paczka:** opcjonalne szyfrowanie całości hasłem. Algorytm do ustalenia w F1 (ADR). **Propozycja:** format `age` z odbiorcą hasłowym (scrypt) albo bezpośrednio **XChaCha20-Poly1305** z kluczem z Argon2id; jeden strumień szyfrujący cały ZIP (`.alfa` zaszyfrowany = nagłówek `ALFAENC1` + parametry KDF + szyfrogram). Manifest paczki zaszyfrowanej jest wewnątrz szyfrogramu; na zewnątrz tylko `kind` i `schema_version` w nagłówku.
- **Eksport sekretów** (`kind = "secrets"`): osobna, jawna operacja; **zawsze** szyfrowana hasłem, tym samym schematem; zawiera `secrets.enc` z wpisami `{ provider_id, account_id, kind, value }`. Import sekretów zapisuje je do Credential Managera maszyny docelowej i nie zostawia jawnych kopii na dysku ani w logach.
- Hasło nie jest zapisywane; utrata = brak dostępu. Kopie zapasowe mogą mieć hasło zapisane w Credential Managerze maszyny (opcja w harmonogramie).

## 6. Import

1. **Walidacja:** rozszerzenie i nagłówek, `schema_version` ≤ obsługiwana (inaczej komunikat „zaktualizuj Alfę"), integralność (`content_sha256`, sumy per plik), reguła ścieżek (§1).
2. **Dry-run (zawsze przed zapisem):** lista elementów z podglądem różnic: nowe / identyczne / różne / kolizja id; liczba wpisów, rozmiar, maszyna źródłowa, ostrzeżenia (nakładka innej klasy sprzętu, brak modułu dla danego elementu, sesje „prywatne").
3. **Tryb** wybierany per kategoria (albo globalnie):
   - `add` — dodaj tylko elementy, których nie ma; kolizje pomijane;
   - `merge` — scal: konfiguracja klucz po kluczu (paczka nadpisuje), sesje dopisują brakujące zdarzenia po `id` (dziennik jest append-only, więc scalanie to suma zbiorów + porządek po `ts`), pamięć dedupluje po `id` i treści;
   - `replace` — zastąp element w całości (sesja, zakres pamięci, plik konfiguracji).
4. **Kolizje id:** sesja o tym samym `id`, ale innym `content_sha256` → wybór: scal / zastąp / importuj jako kopię (nowy `id`, tytuł z sufiksem „(import z <maszyna>)"); wpisy pamięci → dedupe po skrócie treści, konflikty pokazane w dry-run. Id są ULID (do ustalenia w SPEC v1 `sessions`), więc kolizja przypadkowa jest praktycznie niemożliwa — kolizja oznacza tę samą sesję z różnych maszyn.
5. **Migracje:** rekordy o starszej wersji schematu (zdarzenia, konfiguracja, IR sesji) przechodzą przez **upcastery** (łańcuch vN → vN+1) przed zapisem; testy migracji na zamrożonych fixture'ach każdej wersji (PLAN §13, §15.1).
6. **Snapshot przed importem:** automatyczny eksport pełny dotkniętych elementów do `%LOCALAPPDATA%\Alfa\snapshots\<ts>.alfa` + wpis w Audycie; **rollback jednym kliknięciem** = import tego snapshotu w trybie `replace`. Snapshoty rotowane (domyślnie 5).
7. Zapis transakcyjny per element (SQLite w transakcji, pliki przez zapis do tymczasowego + rename). Przerwanie w połowie zostawia elementy nietknięte albo w całości zaimportowane, nigdy w połowie.
8. Wynik: raport (co dodano/scalono/zastąpiono/pominięto), zdarzenie `transfer.import.completed` w Audycie.

Import nakładki maszyny (`config.machine`) tylko na wyraźne życzenie i tylko po ostrzeżeniu, gdy `hw_class` się różni.

## 7. Wersjonowanie i zgodność

- `schema_version` paczki jest niezależna od wersji aplikacji; zmiana łamiąca = major.
- Alfa importuje paczki o wersji ≤ własnej (upcastery) i odmawia nowszych (bez downcastu).
- Każdy rekord w `events.ndjson` niesie własną wersję schematu zdarzenia (PLAN §13) — paczka może mieszać wersje.
- Zestaw fixture'ów `evals/transfer/` po jednej paczce na każdą wydaną wersję schematu; CI: import każdej z nich + round-trip eksport → import → porównanie.

## 8. Kopie zapasowe

- Kopia = **zaplanowany eksport** (`kind = "backup"`) do wskazanego katalogu (lokalny, dysk sieciowy, folder OneDrive), z zakresem jak eksport i rotacją (np. 7 dziennych + 4 tygodniowe; do ustalenia w SPEC v1).
- Harmonogram respektuje reguły tła: nie na baterii, nie w trybie gry/pełnego ekranu (jak konsolidacja pamięci, PLAN §10).
- **Test przywracania w CI**: kopia → czysty profil → import → porównanie z oryginałem (0 różnic w zakresie).
- Harmonogram kopii w F7 (`transfer` pełny); w F1 tylko eksport/import ręczny.

## 9. Przykład `manifest.json`

```json
{
  "schema_version": "1.0.0",
  "app_version": "0.3.1",
  "kind": "export",
  "created_at": "2026-10-04T18:22:07Z",
  "source_machine": {
    "id": "01J9Z8K3M6Q2X7V4W1N5R8T0YB",
    "name": "desktop",
    "os": "Windows 11 Pro 24H2",
    "hw_class": "standard-amd"
  },
  "scope": {
    "keys": ["config.common", "personas", "casts", "sessions"],
    "sessions": ["01J9ZA0V2P8C4D6E1F3G5H7J9K", "01J9ZA1XQ7R2S4T6U8V0W1X3Y5"],
    "counts": { "sessions": 2, "events": 1842, "artifacts": 0, "memory_entries": 0 }
  },
  "content": [
    { "path": "config/common/agents.toml", "sha256": "3f1a…", "bytes": 2048 },
    { "path": "personas/alfa/voice-bible.toml", "sha256": "9b77…", "bytes": 1311 },
    { "path": "sessions/01J9ZA0V2P8C4D6E1F3G5H7J9K/session.json", "sha256": "c0de…", "bytes": 640 },
    { "path": "sessions/01J9ZA0V2P8C4D6E1F3G5H7J9K/events.ndjson", "sha256": "77e1…", "bytes": 512331 }
  ],
  "content_sha256": "aa41…",
  "encryption": null,
  "notes": "Sesje projektu Raport Q3 — do laptopa"
}
```

Dla paczki szyfrowanej `encryption` ma postać `{ "scheme": "xchacha20-poly1305", "kdf": "argon2id", "salt": "…", "nonce": "…" }` (propozycja z §5).

## 10. Otwarte pytania (do SPEC v1 `transfer`)

- Ostateczny wybór algorytmu szyfrowania i biblioteki (ADR w F1).
- Podpis minisign kopii zapasowych: czy wymagany.
- Czy referencje głosowe (próbki audio castingu) wchodzą do `personas/` — zależy od licencji usługi voice design (PLAN §6.6).
- Polityka dla sesji `tainted` i „prywatne" przy eksporcie.
