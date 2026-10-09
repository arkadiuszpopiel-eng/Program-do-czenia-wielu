# 04 — Katalog nowych funkcji

Każda funkcja opiera się na technologii, która istnieje dziś (2026-10-09). Kolumna „Pewność” mówi, jak została
sprawdzona — legenda i źródła: [`11-technologie-2026-10.md`](11-technologie-2026-10.md). **Żadna z tych funkcji
nie jest jeszcze zbudowana ani przetestowana w Alfie**; stan wykonania jest w [`03-roadmapa.md`](03-roadmapa.md).

Sprzęt bazowy: laptop i7-13620H, RTX 4050 z 6 GB pamięci karty, 16 GB RAM.

## Modele i jakość odpowiedzi

| ID | Funkcja | Co daje użytkownikowi | Technologia | Pewność | Na 6 GB |
| --- | --- | --- | --- | --- | --- |
| C2.2a | Lepszy polski model domyślny | wyraźnie lepsze odpowiedzi niż Bielik 4.5B | Bielik-Minitron-7B v3 (GGUF Q4_K_M ≈ 4,5 GB, Apache 2.0) | P | tak; do pomiaru tok/s |
| C2.2b | Model do narzędzi i obrazu | agentki trafniej wywołują narzędzia, rozumieją zrzuty i zdjęcia | Qwen3.5-4B (wizja wbudowana, Apache 2.0) | P (istnienie), W (parametry) | tak (≈ 3,4 GB) |
| C2.2c | Kilka modeli bez restartu | przełączanie czat / narzędzia / embedder bez czekania na ręczne ładowanie | tryb routera w `llama-server` (`--models-max`, usypianie bezczynnych) | P | jeden duży model naraz |
| C2.2d | Tryb „wolniej, ale lepiej” | najtrudniejsze pytania po polsku | Bielik-11B v3 z częściowym liczeniem na procesorze | W | wolno — tylko na życzenie |
| C2.3 | Zawsze poprawne wywołania narzędzi | koniec z „model odpowiedział zepsutym JSON-em” | gramatyki i JSON Schema w llama.cpp | P | tak |
| C2.1 | Zestaw ewaluacyjny po polsku | każda zmiana modelu lub silnika jest mierzona, nie zgadywana | własny zestaw w `evals/`, uruchamiany w CI i na sprzęcie | — | tak |

## Agentki i automatyzacja

| ID | Funkcja | Co daje | Technologia | Pewność |
| --- | --- | --- | --- | --- |
| C2.5 | Łączniki MCP nowej generacji | Alfa korzysta z serwerów MCP pisanych pod nową specyfikację i ze starszych | MCP 2026-07-28 (bezstanowy, `server/discover`) + 2025-11-25; SDK `rmcp` 3.5 | P |
| C2.6 | Umiejętności w otwartym formacie | gotowe „przepisy” dla agentek do pobrania i wymiany; każda z przypiętą sumą i podglądem przed włączeniem | Agent Skills (`SKILL.md`) | W |
| C2.7 | Zadania zaplanowane | „co rano streść pocztę”, „w piątek zrób kopię” — z góry ustalonym zakresem uprawnień | moduły `scheduler` i `triggers` (już w kodzie) + budżet zdolności na zadanie | — |
| C2.8 | Kanwa artefaktów | dokument, strona albo rysunek obok rozmowy, z historią wersji | izolowana ramka bez dostępu do programu (ADR 0009) | — |
| C2.4 | Drugi strażnik treści | dodatkowe wykrywanie szkodliwych poleceń po polsku | Bielik-Guard 0.1B / 0.5B | P (istnienie) |
| C2.11 | Rozmowa o dokumentach | PDF i DOCX jako załącznik lub źródło pamięci | wyciąganie tekstu w Rust; skany — opcjonalnie Docling jako dodatek | W |
| C1.3 | Sterowanie komputerem z asystą | agentka klika i wpisuje, Ty potwierdzasz kroki | drzewo UI Automation + zrzut ekranu jako zapas; Qwen3.5-4B do obrazu | W |

Zasada dla całej grupy: **Broker pozostaje jedynym wykonawcą**. Wbudowane narzędzia `llama-server` (`--tools`,
własna obsługa MCP) pozostają wyłączone — ominęłyby zgody i dziennik.

## Wiele urządzeń

| ID | Funkcja | Co daje | Technologia | Pewność |
| --- | --- | --- | --- | --- |
| D | Dostęp przez Internet bez otwierania portów | Alfa w domu, Ty gdziekolwiek | przekaźnik + kanał szyfrowany end-to-end (wzorzec Noise); dla klientów natywnych opcjonalnie iroh 1.x | P (iroh), U (dobór wzorca) |
| D.2 | Parowanie kodem QR | nowe urządzenie w mniej niż minutę, bez haseł | klucz publiczny + jednorazowy sekret w kodzie; krótki kod kontrolny do porównania | U |
| D.5 | Zgody z telefonu i przeglądarki | zadania nie stoją, gdy nie ma Cię przy komputerze | podpis kluczem urządzenia po biometrii / kluczu dostępu (WebAuthn) | U |
| E | Panel WWW | pełna Alfa w przeglądarce, także na cudzym komputerze | ten sam interfejs Svelte jako PWA | — |
| F | Aplikacja Android | pilot w kieszeni, powiadomienia o zgodach, głos | Tauri 2 mobile (Android stabilny od 2.0) | P (wersja), W (dojrzałość) |
| F.6 | Rozmowa bez komputera | gdy komputer jest wyłączony, rozmawiasz przez ten sam model chmurowy | `offline-client` + magazyn kluczy Androida | — |
| D | Lista urządzeń i odwołanie | zgubiony telefon traci dostęp jednym kliknięciem | `device-pairing` | — |
| D | STOP z każdego klienta | kill-switch działa także zdalnie | istniejący kill-switch + polecenie zdalne o najwyższym priorytecie | — |

## Klucze i konta dla firm

Stan po wersji 0.0.7 (zrobione): diagnoza każdej przyczyny odmowy z odpowiedzią dostawcy, sprzątanie wklejonego klucza,
wymiana klucza bez utraty działającego, wyłączanie konta, wykrywanie tego samego klucza, limit kosztów na konto,
klucz wyłącznie w Menedżerze poświadczeń Windows. Poniżej to, czego jeszcze brakuje do pracy w firmie.

| ID | Funkcja | Co daje | Technologia | Pewność |
| --- | --- | --- | --- | --- |
| C2.12 | Organizacja i projekt | klucz konta firmowego trafia do właściwego projektu i budżetu | nagłówki `OpenAI-Organization` i `OpenAI-Project`; pola opcjonalne w koncie | P |
| C2.12 | Azure OpenAI | firmy z umową Microsoft używają własnego zasobu | adres zasobu, nazwa wdrożenia, wersja API, nagłówek `api-key` | P |
| C2.13 | Serwer pośredniczący i własny certyfikat | Alfa działa w sieci firmowej z inspekcją TLS | ustawienia sieci w `lib-netguard`; certyfikaty z magazynu Windows | W |
| C2.14 | Sprawdzanie kluczy w tle | o wygasłym kluczu lub braku środków wiesz przed rozmową, nie w jej trakcie | `scheduler` + lista modeli (bez kosztu tokenów) | — |
| C2.14 | Wiek klucza | przypomnienie o wymianie po ustalonym czasie | data zapisu klucza przy koncie | — |
| C2.15 | Konto zapasowe | rozmowa nie staje, gdy jedno konto nie ma środków albo jest ograniczane | reguła routera; decyzja widoczna w osi czasu | — |
| C2.16 | Dziennik operacji na kluczach | wiadomo, kto i kiedy dodał, wymienił albo wyłączył klucz | audyt Jądra; nigdy treść klucza | — |
| C2.17 | Import ze zmiennych środowiskowych | administrator rozdaje klucze bez wklejania ich w okno | hub już to umie (`import_from_env`); brakuje polecenia rdzenia i przycisku; zawsze za zgodą użytkownika | — |

Poza zakresem: wspólny magazyn kluczy dla zespołu i logowanie jednokrotne — wymagają serwera, którego Alfa nie ma
(decyzja „lokalnie najpierw”); wrócą razem z hostem bez okna (etap D).

## Zgodność i zaufanie

| ID | Funkcja | Co daje | Podstawa | Pewność |
| --- | --- | --- | --- | --- |
| C2.9 | Oznaczenie „wygenerowane przez AI” | eksport tekstu, dźwięku i obrazu ma czytelną i maszynową etykietę | art. 50 AI Act | W |
| C2.9 | Informacja „rozmawiasz z AI” | przy pierwszym uruchomieniu i stale w interfejsie | art. 50 AI Act | W |
| C2.10 | Lista składników oprogramowania (SBOM) | wiadomo, z czego zbudowano każde wydanie | CRA; `cargo-cyclonedx` + lista npm | W |
| C2.10 | Polityka zgłaszania podatności | jeden adres i jasna procedura | CRA art. 14 | W |
| — | Karty modeli i licencji w programie | przy każdym modelu: skąd, na jakiej licencji, do czego | dobra praktyka | — |

## Ograniczenia, które plan nazywa wprost

| Czego nie obiecujemy | Dlaczego | Co zamiast |
| --- | --- | --- |
| Rozmowy głosowej w pełnym dupleksie po polsku na komputerze bazowym | modele tego typu są anglojęzyczne i potrzebują 16–24 GB pamięci karty | kaskada: wykrywanie mowy → rozpoznawanie → model → synteza, z przerywaniem w pół zdania |
| W pełni samodzielnego sterowania komputerem na 6 GB | otwarte modele GUI (ok. 9 mld parametrów) zajmują całą kartę i ustępują modelom zamkniętym | narzędzia strukturalne i tryb z asystą; pełna autonomia przez model chmurowy |
| Wielu agentek liczących naraz lokalnie | na karcie mieści się jeden duży model | kolejka: planistka i wykonawczynie po kolei, ze wspólną pamięcią podręczną |
| Modelu lokalnego w telefonie | decyzja właściciela; polskiego modelu tej wielkości dziś nie potwierdziliśmy | telefon jako pilot; bez komputera — model chmurowy |
| Rozmycia tła w stylu „Szkło” | reguła projektu zabrania `backdrop-filter` (wydajność WebView2) | półprzezroczyste tafle na barwnej poświacie |
| Pełnej funkcjonalności bez komputera | narzędzia, pamięć i pliki są na komputerze | sama rozmowa przez API; reszta wraca z połączeniem |

## Sprawy do rozstrzygnięcia przed wdrożeniem

| Sprawa | Opcje | Kto |
| --- | --- | --- |
| Silnik mowy: następca Pipera ma licencję GPL-3.0 | osobny proces + opinia prawna / starsze wydanie na MIT / inny silnik | właściciel (ADR 0024) |
| Klucz API w przeglądarce (tryb bez komputera w panelu WWW) | nie przechowywać (tylko Android) / przechowywać zaszyfrowany kluczem dostępu | właściciel (ADR 0023) |
| Wybór rozpoznawania mowy: Whisper turbo czy Parakeet v3 | pomiar WER na polskim korpusie | sesja mierzy, właściciel ustala próg |
| Wybór embeddera i rerankera dla polskiego | pomiar recall@5 na własnym zestawie | jak wyżej |
| Kto prowadzi przekaźnik | własny serwer właściciela / dostawca chmurowy | właściciel |
