# 16 — Plan zbiorczy Alfy (stan i całość planu na 2026-10-11)

Wersja programu **0.0.28**. Ten dokument łączy w jednym miejscu: co jest zrobione, cały dotychczasowy plan v6
(etapy A–H), nowy etap **P** (przeniesienie funkcji z Delty AI) i nowy etap **U** (wielu użytkowników z kontami
i logowaniem) oraz cztery cele: **Windows, Linux, panel WWW, Android** z pełnym sterowaniem i komunikacją.
Szczegóły zadań: [`03-roadmapa.md`](03-roadmapa.md) (dokument żywy). Tu jest obraz całości.

## 1. Czym jest Alfa

Asystentka AI na komputer: cztery agentki (Alfa, Beta, Gama, Delta) rozmawiają tekstem i głosem, pamiętają,
wykonują zadania na plikach i w programach. Trzy zasady, które się nie zmieniają:

1. **Lokalnie najpierw.** Model, pamięć i głos mogą działać bez Internetu; chmura jest wyborem.
2. **Jedno miejsce wykonuje działania.** Każdą akcję w systemie wykonuje Broker — osobny proces ze zgodami,
   dziennikiem i przyciskiem STOP. Agentki nie mają do niego dostępu.
3. **Moduły z kontraktami.** Każdy moduł ma opis, atrapę i testy; platformy i klienci są wymienne.

Decyzja właściciela z 2026-10-11: **Alfa jest programem docelowym**, a z Delty AI 0.8.35 przenosimy system
rozmowy i mowy oraz wszystkie brakujące funkcje.

## 2. Gdzie jesteśmy

| Miara | Stan |
| --- | --- |
| Wersje | 0.0.1 → 0.0.28 w dniach 2026-10-07 → 11; historia w programie: Ustawienia → Aktualizacje |
| Kod | Rust ok. 304 tys. linii w 245 modułach, okno Svelte ok. 56 tys. linii, 227 poleceń rdzenia |
| Testy automatyczne | Rust ok. 2600, okno 311 jednostkowych i 309 w przeglądarce (61 to przegląd układu w 3 stylach × 2 motywach), 17 w prawdziwej aplikacji |
| **Potwierdzone na sprzęcie** | **0 ze 108 kryteriów.** Działa okno, ustawienia, skalowanie; odpowiedź modelu lokalnego, rozmowa głosowa i Broker poza procesem — niesprawdzone |
| Repozytorium | gałąź lokalna, niewypchnięta (czeka na `gh auth login` właściciela) |

Co już jest w programie: rozmowa z gałęziami, cztery agentki z rolami, Kreator agentek, umiejętności, wtyczki,
zadania w tle, pamięć i wyszukiwanie, konta dostawców z kolejkami modeli i kontem zapasowym, mosty CLI, kamery
USB, terminal, pliki i artefakty, eksport `.alfa`, aktualizacje z historią zmian (wagi: czerwona / pomarańczowa /
zielona), Zdrowie systemu, trzy style interfejsu w dwóch motywach, PL/EN.

## 3. Mapa etapów

```
A (Windows MVP) ══▶ P (funkcje Delty) ══▶ C (pozostałe funkcje Windows)
   │                    │
   │                    └▶ U (wielu użytkowników) ─┐
   └▶ B (szwy) ─▶ D (host i protokół zdalny) ◀─────┘
                    ├▶ E (panel WWW)
                    ├▶ F (Android)
        └──────────▶ G (Linux)                    H (badawcze)
```

| Etap | Co daje | Stan | Zadań: zrobione / wszystkie |
| --- | --- | --- | --- |
| A | Windows — pierwsza wersja działająca na sprzęcie („Alfa 0.1”) | w toku | 15 / 21 |
| B | Szwy pod wiele platform i klientów | w toku | 2 / 9 |
| C | Funkcje Windows po MVP (C1–C5) | w toku | 6 / 50 |
| **P** | **Funkcje przeniesione z Delty AI (P0–P11)** | **nowy** | 0 / 15 |
| **U** | **Wielu użytkowników: konta, logowanie, role** | **nowy** | 0 / 10 |
| D | Host bez okna i bezpieczne połączenie zdalne | czeka na ADR 0019–0023 | 0 / 10 |
| E | Panel WWW — sterowanie przez przeglądarkę | planowane | 0 / 7 |
| F | Android 15+ — pilot i tryb bez komputera | planowane | 0 / 8 |
| G | Linux | planowane | 0 / 9 |
| H | Tematy badawcze | bez terminu | — |

## 4. Etap A — Windows, pierwsza wersja

Zrobione: skalowanie 1280–3840 px, style „Szkło” i „Next”, własne kontrolki, belka okna z paskiem LED, nawigacja
Ustawień, klucze API z diagnozą, kolejki modeli, kilka kart graficznych, znak programu, strojenie mowy.

Zostało (wszystko wymaga właściciela albo sprzętu):

| ID | Zadanie | Na co czeka |
| --- | --- | --- |
| A.4 | Pierwsza rozmowa z modelem lokalnym | łączy się z **P2**; pobranie modeli |
| A.5 | Głos na żywo | łączy się z **P1** |
| A.6 | Pomiary na sprzęcie (fale F0–F3) | po A.4 i A.5 |
| A.7 | 44 decyzje bezpieczeństwa | właściciel — pakiet gotowy |
| A.8 | Wdrożenie decyzji, wejście na gałąź główną | po A.7 i `gh auth login` |

## 5. Etap P — przeniesienie z Delty AI

Pełny opis: [`17-przeniesienie-z-delty.md`](17-przeniesienie-z-delty.md). Zasada: z Delty bierzemy zachowanie,
progi zdobyte na sprzęcie i ekrany; piszemy od nowa w Rust, w wyglądzie Alfy, z Brokerem — bez Pythona i bez
wad bezpieczeństwa Delty.

| Grupa | Zakres | Łączy się z |
| --- | --- | --- |
| P0 | Dokumenty: specyfikacja głosu Delty ([`19`](19-specyfikacja-glosu-delty.md)), macierz funkcji ([`20`](20-macierz-delty-czesc-1.md), [`21`](21-macierz-delty-czesc-2.md)) — **gotowe** | — |
| **P1** | **Rozmowa i mowa**: Supertonic 3, 17 głosów, Qwen3-ASR, zabezpieczenia rozpoznawania, Smart Turn, ekran „Rozmowa na żywo” | A.5, C1.1, C5 |
| **P2** | **Modele lokalne**: Sklep modeli, rozkład na karty, dopasowanie do pamięci karty, poprawki szablonów, panel System | A.4, C2.2 |
| P3 | Nawigacja: menu kołowe, doki, okna odczepiane, pulpit kafelkowy, Wstecz / Dalej | — |
| P4 | Rozmowa: karty, Tablica, Tryb skupienia, Arena, załączniki z czytnikami, eksporty | C2.8 |
| P5 | Sieć i wiedza: wyszukiwanie, wiadomości, „Naucz”, polecenia bez modelu, tłumaczenie, instrukcja | C2.6, C2.11 |
| P6 | Osoby: baza osób, rozpoznawanie twarzy i głosu, pamięć per osoba | C3.8–C3.13, **U** |
| P7 | Komputer: sterowanie do poziomu Delty przez Brokera, procesy, urządzenia | C1.3 |
| P8 | Twórczość: Studio obrazu, Galeria, Studio opowieści, Audio | — |
| P9 | Awatar: 2D / 3D / VRM, emocje, ruch ust | — |
| P10 | Odtwarzanie multimediów i sterowanie głosem (bez biblioteki) | — |
| P11 | Przepływy, role i przepływ wieloagentowy, edytor persony, trening, pulpit testowy | C1.5 |
| **P12** | **Agentki programistki**: pisanie, uruchamianie i budowanie oprogramowania z nadrzędnym przyciskiem „Pełne uprawnienia” (jak w Claude Code i Codex), rozmowa między agentkami w trakcie pracy, model dobierany do trudności zadania | C1.2, mosty CLI |
| **P13** | **Dyski sieciowe i chmurowe**: podłączanie (NAS, WebDAV, SFTP, S3, OneDrive, Google Drive, Dropbox), praca agentek na plikach, kopie zapasowe z harmonogramem, wersjami i przywracaniem | C1.4 (transfer) |
| **P14** | **Obowiązki codzienne agentek**: przypisanie agentki do stałych zadań — poczta, kalendarz, przegląd wiadomości, przypomnienia, porządki, raport dnia — z harmonogramem i budżetem | C2.7 |

## 6. Etap C — pozostałe funkcje Windows

| Grupa | Zakres | Stan |
| --- | --- | --- |
| C1 | Z planu v5: jakość głosu, mosty CLI na żywo, sterowanie komputerem, pamięć i transfer, samonaprawa | czeka na sprzęt i decyzje; C1.1 i C1.3 wchodzą w P1 i P7 |
| C2 | Nowe: zestaw ewaluacyjny po polsku, modele na 6 GB, MCP 2026-07-28, Agent Skills, zadania z budżetem, kanwa artefaktów, oznaczanie treści AI (termin 2026-12-02), SBOM, klucze dla firm | 3 z 18 zrobione, 1 w toku (liczby zbiorcze etapu liczy strona startowa HTML) |
| C3 | Kamery i osoby: USB zrobione, sieciowe do zrobienia, opis obrazu w toku, osoby — w P6 | 4 z 15 zrobione |
| C4 | Cztery strony Ustawień ze znakiem „wkrótce” | planowane; spis w dokumencie 14 |
| C5 | Głos lokalny | wchłonięty przez P1 (Supertonic 3 w ustawieniach Delty) |

## 7. Etap U — wielu użytkowników

Pełny opis: [`18-wielu-uzytkownikow.md`](18-wielu-uzytkownikow.md); projekt decyzji: ADR 0027 (Proponowany).

Dziś Alfa zakłada jedną osobę. Docelowo: **konta użytkowników z logowaniem**, na komputerze i zdalnie.

| Element | Opis |
| --- | --- |
| Konta i role | Właściciel (pełnia praw), Dorosły, Dziecko, Gość; każda rola ma własny najwyższy poziom autonomii agentek |
| Logowanie na komputerze | Windows Hello / klucz dostępu albo PIN; szybkie przełączanie; blokada po bezczynności |
| Rozdzielenie danych | osobne rozmowy, pamięć, pliki, klucze API i limity kosztów; dane wspólne tylko za zgodą właściciela |
| Rozpoznanie po głosie i twarzy | wybiera konto do rozmowy (z P6); do ryzykownych działań zawsze logowanie |
| Zdalnie | każde urządzenie sparowane z kontem; panel WWW i telefon logują się na konkretne konto |
| Nadzór | dziennik działań per konto; właściciel widzi zgody i koszty wszystkich; STOP dla każdego |

## 8. Cztery cele: platformy i klienci

Jeden rdzeń (host) i wymienni klienci. Opisy: [`05-windows.md`](05-windows.md), [`06-linux.md`](06-linux.md),
[`07-panel-www.md`](07-panel-www.md), [`08-android.md`](08-android.md),
[`09-dostep-zdalny-i-bezpieczenstwo.md`](09-dostep-zdalny-i-bezpieczenstwo.md).

| Cel | Rola | Pełne sterowanie i komunikacja | Etap | Stan |
| --- | --- | --- | --- | --- |
| **Windows 11** | komputer główny: rdzeń, modele, głos, Broker | wszystko lokalnie | A, P, C | w budowie |
| **Linux** | drugi komputer główny | to samo co Windows; zdolności niedostępne są ukryte | B → G | planowane |
| **Panel WWW** | sterowanie z dowolnej przeglądarki | rozmowa, agentki, pliki hosta, zgody, STOP, podgląd kamer i stanu — 100 % funkcji przez przekaźnik z szyfrowaniem end-to-end | B → D → E | planowane |
| **Android 15+** | pilot w kieszeni | to samo co panel + głos przez mikrofon telefonu, powiadomienia o zgodach, parowanie kodem QR; bez komputera — rozmowa przez model API | B → D → F | planowane |

Jak to się łączy:

- **Komputer nie otwiera portu.** Klienci łączą się przez przekaźnik, który widzi tylko szyfrogram (ADR 0021).
- **Parowanie kodem QR**; lista urządzeń i odwołanie jednym kliknięciem.
- **Zgody zdalne** dla wszystkich poziomów ryzyka, zawsze z potwierdzeniem biometrią albo kluczem dostępu (ADR 0022).
- **Konta (etap U)** obowiązują wszędzie: urządzenie jest sparowane z kontem, a uprawnienia wynikają z roli.
- **Funkcje z etapu P** trafiają do klientów zdalnych przez ten sam zestaw poleceń: rozmowa na żywo z telefonu
  (dźwięk przetwarza host), sklep modeli i panel System zdalnie, podgląd awatara i kamer.

Zrobione pod to wcześniej: numery zdarzeń i wznawianie połączenia (B.2), błędy jako obiekty (B.3), generator
typów (B.1, etap 1).

## 9. Etapy D–H w skrócie

| Etap | Najważniejsze zadania | Wyjście |
| --- | --- | --- |
| D | brama zdalna, parowanie, przekaźnik, kanał E2E, zgoda z urządzenia, host bez okna | drugi komputer prowadzi rozmowę i zatwierdza działanie przez przekaźnik |
| E | układ od 360 px, PWA z tego samego kodu, logowanie kluczem dostępu, powiadomienia | wszystko, na co pozwala host, da się zrobić z przeglądarki |
| F | aplikacja Tauri mobile, magazyn kluczy i biometria, pilot 100 %, głos w telefonie, tryb bez komputera | telefon sparowany w minutę; rozmowa działa także po wyłączeniu komputera |
| G | porty platformy Linux, analizator powłoki POSIX, Jądro jako usługa, PipeWire, pakiety deb / rpm / Flatpak | scenariusz MVP na Ubuntu z kartą NVIDIA |
| H | pełny dupleks głosowy, model w telefonie, NPU, MCP Apps, kandydaci z planów Delty (klonowanie głosu Chatterbox, śledzenie twarzy dla awatara) | bez zobowiązania |

## 10. Kolejność prac

1. **P0** — dokumenty przeniesienia (macierz funkcji, specyfikacja głosu) — gotowe, do przeglądu właściciela.
2. **P1** — rozmowa i mowa na żywo na laptopie → zamyka A.5.
3. **P2** — modele lokalne → zamyka A.4; potem pomiary A.6.
4. **P3–P5** — nawigacja, rozmowa, sieć i wiedza.
5. **U (pierwsza część)** — konta i logowanie na komputerze; **P6** — osoby (korzysta z kont).
6. **P12** — agentki programistki (po P2, bo korzysta z doboru modeli); **P7–P11** — komputer, twórczość, awatar, multimedia, pozostałe.
7. **B** (reszta) → **D** → **E** i **F** → **U (część zdalna)** → **G**.

Zadania C2 z terminami prawnymi (oznaczanie treści AI do 2026-12-02, SBOM przed wydaniem publicznym) wchodzą
niezależnie od tej kolejności.

## 11. Decyzje właściciela

Podjęte 2026-10-11: Alfa programem docelowym · głos lokalny, nie z chmury · głos Edge „Zofia” tylko jako opcja
wyłączona domyślnie · zgoda na pobrania do P1 · ADR 0025 (dane biometryczne) przyjęty · bez biblioteki
multimediów, z systemem odtwarzania · wielu użytkowników z kontami i logowaniem · agentki mają pisać
i uruchamiać oprogramowanie z pełnymi uprawnieniami w projekcie i dobierać model do zadania · dyski sieciowe i chmurowe z pełnymi uprawnieniami do kopii zapasowych · agentki przypisywane do
obowiązków codziennych (poczta i podobne) · stała zgoda na
pobrania do 3 GB.

Nadal otwarte:

| Decyzja | Blokuje |
| --- | --- |
| `gh auth login` i wypchnięcie commitów | A.0, A.8 |
| 44 decyzje bezpieczeństwa | A.7, A.8, B.7 |
| Podpis pod ADR 0017–0018 | B.4, B.7, G |
| Podpis pod ADR 0019–0023 | D, E, F |
| Podpis pod ADR 0027 (konta i role) | U |
| Pobieranie multimediów z serwisów | część P10 |
| Progi kryteriów akceptacji | wyjście z każdego etapu |

## 12. Jak pracujemy

- Każda seria zmian to wyższa wersja z wpisem w dzienniku zmian i wagą każdej pozycji.
- Każdy ekran sprawdzany w trzech stylach i dwóch motywach, w stanie otwartym, i w prawdziwej aplikacji.
- Nic nie jest „sprawdzone”, jeśli nie było uruchomione; kryterium zalicza dopiero wynik ze sprzętu.
- Pobieranie modeli i silników tylko za zgodą właściciela, z podaniem rzeczywistego rozmiaru.
- Ten dokument i roadmapa są aktualizowane po każdym etapie; wersja HTML powstaje z plików MD.
