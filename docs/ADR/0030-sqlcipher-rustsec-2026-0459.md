# ADR 0030 — naprawiony SQLCipher bez wyjątku dla RUSTSEC-2026-0459

Status: propozycja w draft PR #24; wymaga niezależnego przeglądu przed scaleniem.
Data: 9.11.2026. Zakres: istniejąca biblioteka szyfrowanej bazy, bez nowej funkcji programu.

## Problem

CI167 wykrywa RUSTSEC-2026-0459 / CVE-2026-119375 w libsqlite3-sys0.38.2.
Alfa używa `bundled-sqlcipher-vendored-openssl`; rzeczywisty silnik to SQLite3.51.3 / SQLCipher4.14.
Samo podniesienie rusqlite do0.41.1 i adaptera do0.39.1 nie usuwa podatności tej konfiguracji:
adapter nadal przypina sqlcipher-src419.0.6 ze starszym SQLite3.53.4.

## Decyzja proponowana

- rusqlite0.41.1, z dotychczasowymi cechami `cache` i `bundled-sqlcipher-vendored-openssl`;
- lokalna kopia opublikowanego libsqlite3-sys0.39.1 z jedną zmianą zależności:
  `sqlcipher-src =420.0.0`, czyli SQLCipher4.20 / SQLite3.54.1;
- identyczny adapter dla workspace i osobnej powłoki Tauri, zaktualizowane oba lockfile;
- bez zmiany deny.toml, wyjątków advisory, dozwolonych źródełGit, szyfrowania lub formatu klucza.

Kopia pochodzi z oficjalnego archiwum crates.io o SHA256
`14a5f01b87b92705d07b433c06f96cbc764637d81a0bffb5734ce17cf0f9a2b3`.
Generator `scripts/vendor-sqlite.mjs` sprawdza pin przed rozpakowaniem, zachowuje licencję,
aktualizuje oba manifesty upstream, rozdziela pięć istniejących modułów build.rs bez zmiany ich
logiki i zachowuje treść wiązań bindgen w katalogach `generated`. Normalizuje wyłącznie końcowe białe
znaki w czterech plikach upstream; manifest zawiera odciski przed i po tej operacji.
Pozostałe plikiRust mieszczą się w400 liniach; nie dodaje się wyjątku rozmiaru.
Wynik ma manifest SHA256 wszystkich plików i sprawdzenie offline w `pnpm lint`.
LF w `.gitattributes` zapobiega zmianom odcisków przy pobraniu na Windows.

Kopia wspiera używaną przez Alfę konfigurację SQLCipher. Pominięto nieużywane źródła zwykłego
SQLite oraz obcy Cargo.lock; właścicielem wyboru wersji pozostają lockfile Alfy.
Nie udostępniamy tu wariantu zwykłej nieszyfrowanej bazy. KodC SQLCipher pochodzi z przypiętego
pakietu sqlcipher-src420 na crates.io, z sumą z Cargo.lock; nie z obcej gałęziGit.
Kopia adaptera jest wyłączona z członków workspace jako zależność zewnętrzna, nie jako moduł Alfy.

## Walidacja i ograniczenia

Regresja sprawdza rzeczywiste `sqlite_version()` i `cipher_version`, a nie sam numer pakietu.
Stary silnik daje RED na minimalnym SQLite3.54.0. Naprawiony ma zachować szyfrowany nagłówek,
odrzucanie błędnego klucza, FTS5/sqlite-vec, migracje, zapis i role rozmów.
Ręczna fixture zachowuje syntetyczną zaszyfrowaną bazę oraz aktywnyWAL utworzone starym silnikiem;
nowy musi odczytać zapis. Nie używa danych ani sekretów właściciela.

Przed scaleniem wymagane są: pełny audyt bez wyjątku, RustLinux/Windows, powłoka Tauri i WebView2,
niezależny przegląd. Wyniki [w raporcie](../reviews/2026-11-09-ci167-sqlite.md).
To nie jest potwierdzenie wykorzystania CVE w Alfie ani odbiór sprzętu/mikrofonu właściciela.

## Oddzielna decyzja licencji

Aktualny audyt odrzuca w sqlcipher-src420 identyfikatory `blessing` i `WTFPL`.
Osobna, zatwierdzona przez właściciela łatka dopuszcza te licencje tylko dla tego pakietu i wersji;
pozytywny audyt na hipotetycznej konfiguracji nie jest akceptacją aktualnej polityki.
Właściciel9.11 o16:47UTC zatwierdził wyłącznie łatkę51e27c10. Zastosowano ją bez innych
zmian polityki; audyt rzeczywistej konfiguracji zaliczył wszystkie4 kontrole. Nie zmieniono
deklaracji upstream i nie zignorowano podatności. Nie zastępuje to niezależnego przeglądu kodu.

## Konsekwencje

Utrzymujemy małą, jawną różnicę wobec źródła upstream do czasu opublikowania zgodnego adaptera
z naprawionym SQLCipher. Odtwarzanie wymaga oryginalnego archiwum i rustfmt; kontrola istniejącej
kopii jest offline. Po wydaniu zgodnej wersji upstream można usunąć kopię osobnym przyrostem
z tymi samymi regresjami. Nie zmieniamy progów evals ani wersji programu w tym etapie.
