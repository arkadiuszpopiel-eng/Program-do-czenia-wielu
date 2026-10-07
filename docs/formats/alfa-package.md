# Format paczki `.alfa` (moduł `transfer`, PLAN §15.1)

Szkic v0. Paczka `.alfa` służy do **przenoszenia danych między maszynami właściciela** (import/eksport) i do **kopii zapasowych**. Nie ma automatycznej synchronizacji (PLAN §19). Ten sam format i ten sam kod obsługują eksport ręczny, eksport pojedynczej sesji z panelu Sesje i zaplanowane kopie zapasowe.

## 1. Kontener

- **Archiwum ZIP** (deflate; duże pliki artefaktów mogą być `store`). Rozszerzenie `.alfa`; typ MIME `application/vnd.alfa.package+zip` (do ustalenia w SPEC v1 `transfer`).
- Pierwszym wpisem archiwum jest `manifest.json` (nieskompresowany) — pozwala na podgląd bez rozpakowania całości.
- Wszystkie ścieżki wewnątrz względne, `/` jako separator, bez `..`, bez ścieżek absolutnych (import odrzuca paczkę naruszającą tę regułę — ochrona przed zip-slip). **Doprecyzowanie (F1):** odrzucane są też `\`, `:` (litera dysku, strumienie NTFS), UNC, segmenty puste, `.`/`..`, nazwy zarezerwowane Windows (`CON`, `NUL`, `COM1`…, także z rozszerzeniem), segmenty kończące się kropką lub spacją, znaki sterujące; ścieżka ≤ 512 B, ≤ 16 segmentów (`transfer_contract::validate_entry_path`). Archiwum nie może zawierać katalogów, dowiązań, wpisów szyfrowanych ZIP ani powtórzonych nazw; każdy wpis musi być w `content` i odwrotnie.
- **Limity odczytu (zip-bomb):** ≤ 200 000 wpisów, wpis ≤ 2 GiB, łącznie ≤ 16 GiB, manifest ≤ 64 MiB, stopień kompresji wpisu > 1 MiB ≤ 1000; odczyt wpisu ograniczony do rozmiaru z manifestu i weryfikowany SHA-256.
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
  session.json                      nagłówek: { v, meta, workdir_rel, active_leaf, draft, turns, turns_sha256 }
  turns.ndjson                      drzewo tur (append-only): jedna tura na linię { v, id, parent, branch, role,
                                    author, content, usage, created_at, heard_prefix, hidden }, rosnąco po id
  attachments/                      załączniki (później)
memory/<scope>/<id>.ndjson          wpisy pamięci z proweniencją (P1; zakresy: sesja/projekt/globalna/agentka)
artifacts/<session-id>/             pliki oddane przez agentki (opcjonalnie)
logs/                               logi (domyślnie NIE eksportowane)
secrets.json                        NIE POWSTAJE (CX-a, §5); w paczkach starszych wersji — przy imporcie pomijany/odrzucany
rollback.json                       TYLKO w snapshocie: { v, created[], saved[] } — elementy utworzone/zapisane przez import
```

Katalogi nieużyte w danym zakresie po prostu nie występują. Import ignoruje nieznane katalogi, ale wypisuje je w dry-run jako „pominięte".

**Doprecyzowanie (F1):** sesja to nagłówek + `turns.ndjson` (zamiast `events.ndjson` + `branches.json`) — drzewo tur z `parent`/`branch` jest pełną historią z gałęziami (projekcje liczy moduł `sessions`). Identyfikator sesji (nazwa katalogu) `[A-Za-z0-9_-]{1,64}`. `meta.workdir` i `workdir_rel` są **względne** wobec korzenia katalogów roboczych (`%USERPROFILE%\Alfa\Sesje`) — paczka nie zawiera nazwy konta Windows; import składa ścieżkę z lokalnym korzeniem. Dokumenty kategorii: `config/common/*.toml`, `config/machine/<id>.toml`, `personas/*` (np. `personas.json` = `PersonasExport`), `casts/*`, `rules/*`, `skills/*`, `memory/<zakres>.ndjson`, `artifacts/<sesja>/…`, `logs/*`.

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
| `kind` | `export` \| `backup` \| `snapshot` (`secrets` — tylko w paczkach starszych wersji; import odmawia, §5) |
| `notes` | opcjonalny opis od użytkownika |
| `redactions` | liczba zredagowanych ciągów wyglądających na sekrety (strażnik eksportu, §4) |

`content_sha256` = SHA-256 nad liniami `ścieżka\tsha256\tbajty\n` posortowanymi po ścieżce. `scope.counts`: `sessions`, `turns`, `documents`, `memory_entries`, `artifacts`, `secrets` (zawsze 0 — pole zgodności). `source_machine.name` to etykieta nadana przez użytkownika (nie nazwa hosta ani konta).

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

- **Paczka:** opcjonalne szyfrowanie całości hasłem. **Rozstrzygnięte (F1):** `ALFAENC1` (8 B) ‖ długość nagłówka (u32 LE) ‖ nagłówek JSON `{ format: 1, kind, schema_version, scheme: "xchacha20poly1305-stream-be32", chunk: 65536, kdf: { alg: "argon2id", m_kib, t, p, salt } | { alg: "machine-key", key }, nonce_prefix }` ‖ szyfrogram. Szyfrowanie: **XChaCha20-Poly1305 w konstrukcji STREAM** — fragmenty 64 KiB jawnego tekstu, nonce = prefiks 19 B ‖ licznik u32 BE ‖ flaga ostatniego fragmentu; cały nagłówek (od `ALFAENC1`) jest AAD każdego fragmentu (zmiana nagłówka lub obcięcie = błąd). Klucz: **Argon2id** (m = 46 MiB, t = 2, p = 1, sól 16 B; przy odczycie m ≤ 256 MiB, t ≤ 16, p ≤ 8). Odczyt losowy (fragment po fragmencie) — ZIP czytany wprost z szyfrogramu. Manifest paczki zaszyfrowanej jest wewnątrz szyfrogramu; na zewnątrz tylko `kind` i `schema_version` w nagłówku. Hasło ≥ 8 znaków.
- **Snapshot** przed importem: szyfrowany losowym kluczem maszyny z Credential Managera (`Alfa/transfer/snapshot-key`, `kdf.alg = "machine-key"`).
- **Sekrety nigdy w paczce** (AGENTS.md: sekrety tylko w Windows Credential Manager — decyzja CX-a z 2026-10-04, AGENTS.md wygrywa z wcześniejszym PLAN §15.1). Eksportu sekretów nie ma. Zgodność odczytu: paczka `kind = "secrets"` ze starszej wersji → odmowa podglądu i importu z komunikatem „dodaj klucze ponownie w Ustawienia → Konta”; wpis `secrets.json` w innej paczce → pominięty z ostrzeżeniem (nic nie trafia do Credential Managera). Klucze na nowej maszynie dodaje się ręcznie (kreator kont) albo importem ze zmiennych środowiskowych.
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
- Alfa importuje paczki o wersji ≤ własnej (upcastery) i odmawia nowszych (bez downcastu). Starsze „major” bez
  upcastera (np. `0.x` — nigdy niewydane; v0 to `format: 0`) → czytelna odmowa `older_schema` („starszy niż
  najstarszy obsługiwany”), nie „zaktualizuj Alfę”; wersja jest sprawdzana przed odczytem struktury manifestu
  (`transfer_contract::check_schema_version`, `OLDEST_SCHEMA_VERSION`; fala 5, m-06).
- Każdy rekord w `turns.ndjson` (i nagłówek `session.json`) niesie własną wersję (`v`) — paczka może mieszać wersje; upcastery v0 → v1 (`transfer_contract::migrate`: manifest `{ format: 0, files[] }`, nagłówek `{ v: 0, id, title, created }`, tura `{ v: 0, id, parent, role, text, ts }` bez gałęzi — gałęzie liczone regułami `TreeCursor`).
- Zestaw fixture'ów `evals/transfer/` po jednej paczce na każdą wydaną wersję schematu; CI: import każdej z nich + round-trip eksport → import → porównanie.

## 8. Kopie zapasowe

- Kopia = **zaplanowany eksport** (`kind = "backup"`) do wskazanego katalogu (lokalny, dysk sieciowy, folder OneDrive), z zakresem jak eksport i rotacją. **F1:** nazwa `alfa-backup-RRRRMMDD-GGMMSS-mmm.alfa`, rotacja = N ostatnich (pliki o innych nazwach nietknięte); 7 dziennych + 4 tygodniowe — F7.
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
    "counts": { "sessions": 2, "turns": 1842, "documents": 2, "memory_entries": 0, "artifacts": 0, "secrets": 0 }
  },
  "content": [
    { "path": "config/common/agents.toml", "sha256": "3f1a…", "bytes": 2048 },
    { "path": "personas/alfa/voice-bible.toml", "sha256": "9b77…", "bytes": 1311 },
    { "path": "sessions/01J9ZA0V2P8C4D6E1F3G5H7J9K/session.json", "sha256": "c0de…", "bytes": 640 },
    { "path": "sessions/01J9ZA0V2P8C4D6E1F3G5H7J9K/turns.ndjson", "sha256": "77e1…", "bytes": 512331 }
  ],
  "content_sha256": "aa41…",
  "encryption": null,
  "notes": "Sesje projektu Raport Q3 — do laptopa",
  "redactions": 0
}
```

Dla paczki szyfrowanej `encryption` ma postać `{ "scheme": "xchacha20-poly1305", "kdf": "argon2id", "salt": "…", "nonce": "…" }` (propozycja z §5).

## 10. Otwarte pytania (do SPEC v1 `transfer`)

- ~~Ostateczny wybór algorytmu szyfrowania i biblioteki~~ — rozstrzygnięte w F1 (§5; RustCrypto `chacha20poly1305` 0.10, `argon2` 0.5); ADR do zatwierdzenia przez człowieka.
- Podpis minisign kopii zapasowych: czy wymagany.
- Czy referencje głosowe (próbki audio castingu) wchodzą do `personas/` — zależy od licencji usługi voice design (PLAN §6.6).
- Polityka dla sesji `tainted` i „prywatne" przy eksporcie.
