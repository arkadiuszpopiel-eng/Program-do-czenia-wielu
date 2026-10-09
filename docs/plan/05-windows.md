# 05 — Windows 11 (pierwsza wersja)

Windows 11 x64 jest jedyną platformą, na której Alfa dziś działa, i celem pierwszej wersji. Nowsze wydania
Windows mają działać bez zmian w rdzeniu — wszystko, co systemowe, jest w siedmiu crate'ach
`platform-windows-*-impl` i w `voice-audio-impl`.

## Zakres pierwszej wersji („Alfa 0.1”)

Definicja MVP: [`../ACCEPTANCE.md`](../ACCEPTANCE.md) §2 — scenariusz sześciu kroków bez kluczy API. Zakres to
fale F0–F3 planu v5: rozmowa z modelem lokalnym, głos podstawowy, cztery agentki, Broker z oknem zgód,
watchdog, kill-switch, „Cofnij”, narzędzia plików, powłoki i schowka.

| Element | Stan kodu | Stan na sprzęcie |
| --- | --- | --- |
| Uruchomienie, okna, zasobnik, skalowanie 1280–3840 px | jest | ✅ laptop (testy `e2e-app` 11/11 na prawdziwej aplikacji) |
| Style interfejsu: Klasyczny, Szkło, Next (jasny i ciemny) | jest | obejrzane w przeglądarce na atrapie; w prawdziwej aplikacji — po przebudowie 0.0.3 |
| Rozmowa z modelem lokalnym | jest | **nie sprawdzone** — brak silnika na laptopie (czeka na pobranie pakietu) |
| Głos: dyktowanie, czytanie, rozmowa | jest | **nie sprawdzone** — jw. |
| Broker poza procesem, okno zgód, watchdog | jest | **nie sprawdzone** na żadnej maszynie (tryb deweloperski: Broker w procesie) |
| Instalator NSIS, aktualizacje side-by-side | jest | nie sprawdzone lokalnie |
| Narzędzia agentek z „Cofnij” | jest | testy na prawdziwych procesach (m.in. kodowanie wyjścia powłok) |

Pełna macierz: [`../STATUS.md`](../STATUS.md); streszczenie: [`00-stan-wykonania.md`](00-stan-wykonania.md).

## Wymagania sprzętowe

| Klasa | Przykład | Co działa |
| --- | --- | --- |
| Minimalna | procesor 4 rdzenie, 8 GB RAM, bez karty | rozmowa z małym modelem na procesorze albo przez chmurę; głos wolny |
| Bazowa (laptop CUDA) | i7-13620H, RTX 4050 6 GB, 16 GB RAM | model 7B na karcie, głos, agentki z narzędziami |
| Desktop AMD | karta AMD przez Vulkan | jak bazowa; rozpoznawanie mowy na procesorze — do pomiaru |
| Mocna | karta 12–24 GB | modele 11B i większe, model obrazu obok czatu |

Pakiety modeli „od 6 (wzorcowy) do 1 (minimalny)” w Ustawienia → Modele i silniki dobierają zestaw do klasy.

## Co jeszcze trzeba zrobić na Windows

Kolejność i stan: [`03-roadmapa.md`](03-roadmapa.md), etapy A, B, C.

1. **Zamknąć MVP na sprzęcie** (A.4–A.8): pierwsza rozmowa i głos na żywo, pomiary, decyzje bezpieczeństwa,
   przegląd Jądra przez właściciela.
2. **Szwy** (B): zmiany niewidoczne dla użytkownika, które przygotowują inne platformy.
3. **Funkcje po MVP** (C): z planu v5 (jakość głosu, mosty, sterowanie komputerem, pamięć, samonaprawa)
   i nowe (modele v3, MCP 2026-07-28, umiejętności, zadania zaplanowane, kanwa, oznaczenia AI, SBOM).

## Specyfika Windows, która zostaje

| Obszar | Rozwiązanie | Uwagi |
| --- | --- | --- |
| Okno i tło | WebView2; tło przez natywną Mica (własny pasek tytułu — planowany, C1.5) | `backdrop-filter` zabroniony |
| Audio | WASAPI (crate `wasapi`), AEC z własną referencją | ADR 0011 |
| Sekrety | Windows Credential Manager | jedyne miejsce na klucze |
| Jądro | usługa `AlfaBroker` albo tryb przenośny; potok nazwany z DACL; okno zgód na podwyższonej integralności | ADR 0003 |
| Zgody lokalne | fizyczne kliknięcie albo Windows Hello | zostaje obok zgód zdalnych (ADR 0022) |
| Instalacja | NSIS na użytkownika, wersje obok siebie, podpis minisign | ADR 0007; brak Authenticode |
| Skróty | reguła AltGr; kill-switch `Ctrl+Shift+F12` | ADR 0010 |
| Powłoki | PowerShell i `cmd` z analizatorem poleceń | otwarte ustalenia S-01…S-04, S-06 |

## Windows jako host dla innych klientów

Po etapie D komputer z Windows jest „domem” Alfy: trzyma dane, modele i Brokera, a telefon i przeglądarka łączą
się z nim przez przekaźnik. Wymagania wobec wersji Windows:

- Alfa może działać w tle (zasobnik) i startować z systemem — opcja, domyślnie wyłączona.
- Ustawienia → Urządzenia: parowanie, lista, odwołanie.
- Komputer uśpiony = host niedostępny. Opcja „nie usypiaj, gdy urządzenia są połączone” (port `PowerPort` już
  istnieje) — domyślnie wyłączona; klient pokazuje wtedy tryb bez komputera.

## Przyszłe wersje Windows i ARM64

- Nowsze wydania Windows 11: ryzyko po stronie WebView2 (aktualizuje się samo) i UI Automation. Zabezpieczenie:
  testy `e2e-app` na prawdziwej aplikacji po każdej dużej aktualizacji systemu.
- Warstwa agentowa Windows (rejestr MCP w systemie, obszar roboczy agentów) jest w podglądzie — obserwujemy;
  ewentualna integracja to osobny ADR.
- ARM64 pozostaje poza zakresem (plan v5). Po etapie B.6 dodanie architektury to nowe wiersze w katalogu
  silników i budowa — bez zmian w rdzeniu.

## Kryteria akceptacji

Bez zmian: 108 kryteriów F0–F9 z `ACCEPTANCE.md`. Nowe funkcje C2 dostaną kryteria `ACC-V6-…` — definicje pisze
sesja-recenzentka, progi ustala właściciel.
