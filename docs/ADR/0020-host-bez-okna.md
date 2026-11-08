# ADR 0020 — Host bez okna: binarka `alfa-host`, okno Tauri jako klient lokalny

| Pole | Wartość |
|---|---|
| Status | Proponowany — czeka na podpis właściciela |
| Data | 2026-10-09 |
| Powiązane | plan v6: `docs/plan/02-architektura-docelowa.md` (szew 4), `09-dostep-zdalny-i-bezpieczenstwo.md` (test 6), `03-roadmapa.md` (D.1, D.7, D.8); ADR 0001, 0003, 0007, 0019, 0021 |
| Zmienia | założenie, że host żyje wyłącznie wewnątrz procesu okna Tauri; nie zmienia Brokera, watchdoga ani launchera |

## Kontekst

Host to proces z `AppCore`, modułami i Brokerem. Dziś żyje wewnątrz okna Tauri: zamknięcie programu kończy
hosta, a więc i dostęp z telefonu lub przeglądarki. `AppCore::build(paths, options)` działa już bez Tauri
(`shell: None`, `HeadlessShell`), ale nie istnieje plik wykonywalny, który by go tak uruchomił. Dostęp zdalny
(ADR 0021) ma sens tylko wtedy, gdy host może działać, kiedy nikt nie patrzy na okno. Jednocześnie wersja
Windows jest przed zamknięciem MVP i nie wolno jej ryzykować dużą przebudową procesu.

## Decyzja

| Element | Propozycja |
|---|---|
| Krok 1 — brama w procesie aplikacji | brama klientów (`remote-gateway`) działa **wewnątrz** obecnego procesu aplikacji; dostęp zdalny działa, gdy Alfa jest uruchomiona; układ procesów bez zmian |
| Krok 2 — wydzielenie hosta | moduł `host-headless` daje binarkę `alfa-host`, która buduje `AppCore` bez okna i uruchamia bramę klientów zdalnych |
| Start | przy logowaniu użytkownika — opcjonalnie, decyzją w Ustawieniach |
| Zasobnik | ikona w zasobniku przez okno Tauri uruchamiane na żądanie |
| Okno Tauri po kroku 2 | staje się jednym z klientów hosta — lokalnym, przez nazwany potok — zamiast jego właścicielem; używa tego samego protokołu (ADR 0019) |
| Nasłuch sieciowy | brak: host nie otwiera portu; statyczna reguła „żadnego nasłuchu” obejmuje `host-headless`, `remote-gateway` i `device-pairing` |
| Kolejność | krok 2 dopiero po tym, jak krok 1 przejdzie testy z etapu D; do tego czasu `alfa-host` nie trafia do wydania |
| Zgody lokalne | bez zmian: okno Brokera i dowód fizycznego wejścia; host bez okna nie zatwierdza niczego sam |

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Od razu wydzielić hosta (bez kroku 1) | przebudowa procesu przed zamknięciem MVP ryzykuje wersję Windows; krok 1 daje dostęp zdalny bez zmiany układu procesów |
| Zostać na stałe przy bramie w procesie okna | dostęp zdalny znika po zamknięciu okna; komputer nie może być „domem” Alfy |
| Okno ukryte zamiast hosta bez okna | WebView2 w pamięci bez potrzeby; awaria interfejsu kończyłaby hosta |
| Host jako usługa systemowa (sesja 0) | host potrzebuje sesji użytkownika (audio, schowek, okna, modele na GPU); usługą w sesji 0 pozostaje tylko Broker (ADR 0003) |
| Okno łączy się z hostem przez lokalny port TCP | łamie regułę „żadnego nasłuchu” (S14); nazwany potok z kontrolą tożsamości już jest w projekcie |

## Konsekwencje

- Po kroku 2 okno Tauri przestaje wołać `AppCore` bezpośrednio. Plan nie rozstrzyga, czy potok lokalny jest
  czwartym transportem obok Tauri IPC, kanału szyfrowanego i atrapy, czy jest ukryty za poleceniami Tauri —
  do ustalenia w SPEC `host-headless`.
- Launcher, aktualizator i watchdog muszą znać nowy proces: kto go uruchamia, kto restartuje, jak przebiega
  aktualizacja wersji obok siebie (ADR 0007). To obszar wymagający przeglądu człowieka (`watchdog`, `updater`).
- Kill-switch musi zatrzymywać hosta bez okna tak samo jak dziś aplikację; STOP z każdego klienta jest w planie
  jako polecenie o najwyższym priorytecie.
- Polecenia `local_only` (okna dialogowe, podgląd plików) zostają w oknie — host bez okna ich nie obsłuży,
  dlatego oznaczenie z ADR 0019 jest warunkiem kroku 2.
- Zużycie pamięci hosta działającego w tle bez okna trzeba zmierzyć na komputerze bazowym; próg ustala właściciel.
- Na Linuksie ta sama binarka jest punktem wyjścia dla hosta; sposób startu (usługa użytkownika) jest zadaniem
  etapu G i nie jest tu rozstrzygany.

## Jak cofnąć

- Krok 1 jest wyłączalny ustawieniem: bez włączonej bramy aplikacja działa jak dziś.
- Krok 2 jest odwracalny, dopóki okno Tauri potrafi zbudować `AppCore` we własnym procesie — ta ścieżka zostaje
  w kodzie co najmniej do czasu, aż `alfa-host` przejdzie kryteria na sprzęcie.
- Do podpisu tego ADR host żyje wyłącznie w procesie okna.
