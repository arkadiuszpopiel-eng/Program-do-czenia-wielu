# Decision Log — Alfa AI Program

<!-- AUTO-ŁADOWANY przez SessionStart hook. -->
<!-- Dodawaj wpisy po każdej ważnej decyzji architektonicznej lub technicznej. -->
<!-- Format ADR (Architecture Decision Record) lite. -->

## Szablon wpisu

```markdown
### [YYYY-MM-DD] Tytuł decyzji

**Kontekst:** Dlaczego musieliśmy coś zdecydować?
**Opcje:** A (zalety/wady) vs B (zalety/wady)
**Decyzja:** Opcja X
**Powód:** Główna przyczyna
**Konsekwencje:** Co to zmienia/ogranicza
**Status:** Aktywna | Pod rewizją | Cofnięta
```

---

## Stack & Architektura

<!-- Decyzje fundamentalne, rzadko zmieniane -->

<!-- Przykład:
### [2026-01-15] Baza danych: PostgreSQL
**Kontekst:** Potrzebny relacyjny storage z JSONB.
**Decyzja:** PostgreSQL 16
**Powód:** JSONB + ACID + doświadczenie zespołu
**Status:** Aktywna
-->

---

### [2026-10-09] Wiele platform i dostęp zdalny (plan v6)

**Kontekst:** Właściciel chce Alfy na Windows 11 (pierwsza wersja), potem Linux, panel WWW przez Internet i Android 15+.
**Opcje:** przekaźnik z E2E / własny tunel użytkownika / oba; zgody zdalne: tylko podgląd / niskie i średnie / wszystkie;
klienci: tylko pilot / pilot + synchronizacja / pełna synchronizacja.
**Decyzja:** przekaźnik + szyfrowanie end-to-end (komputer nie otwiera portu, parowanie QR); zgody zdalne dla
wszystkich poziomów ryzyka; telefon i panel WWW jako pilot 100 % funkcji, bez komputera — rozmowa przez ten sam model
chmurowy (API), model lokalny tylko na komputerze; plan v6 obok planu v5.
**Powód:** wybór właściciela (odpowiedzi z 2026-10-09).
**Konsekwencje:** zmienia ustalenia planu v5 (tylko Windows, bez synchronizacji, sekrety tylko na komputerze, zgoda
tylko fizyczna) — projekty ADR 0017–0024 w `alfa/docs/ADR/`, do podpisu właściciela przed powstaniem kodu.
**Status:** Aktywna (ADR-y: Proponowane)

---

## Decyzje implementacyjne

<!-- Mniejsze decyzje (biblioteki, wzorce, konwencje) -->

---

## Anty-wzorce

<!-- Co świadomie odrzucaliśmy i dlaczego — żeby nie wracać do tego za miesiąc -->

## 2026-10-11 — zgoda właściciela na cztery decyzje po wersjach 0.0.19–0.0.22

Właściciel napisał: „Decyzje dla Ciebie od punktu 1 do 4 — masz zgodę na pracę”. Dotyczy:

1. Kamery sieciowe (C3.5): wolno dodać bibliotekę do dekodowania obrazu z kamer i pracować nad wyszukiwaniem
   kamer w sieci lokalnej (moja interpretacja: zgoda obejmuje pobranie zależności z crates.io i zakres „sieć lokalna”;
   nie obejmuje modeli AI ani zmian w `lib-netguard` bez przeglądu — to nadal obszar właściciela).
2. Generator typów (B.1): `schemars` + własny skrypt zostaje zamiast `tauri-specta`/`ts-rs`.
3. Etap 2 generatora: wybór sposobu (wyliczenia w Rust albo zawężanie w oknie) należy do mnie.
4. Zestaw `evals/V6/pl-core`: zaakceptowany do dalszej pracy; progów liczbowych właściciel nie podał — proponuję
   je i pokazuję do potwierdzenia.

## 2026-10-11 — głos: lokalnie, nie z chmury

Właściciel: głos przez API będzie za drogi; kazał zbadać silniki lokalne i konkurencję, potem zaakceptował
rekomendację („OK akceptuję rekomendacje twoje, wykonaj to najbardziej profesjonalnie”).
Rekomendacja: dwa poziomy głosu lokalnego — „Naturalny” (Chatterbox Multilingual V3, MIT, przez `chatterbox.cpp`)
i „Szybki” na procesorze (Supertonic 3 po próbie; do tego czasu Piper). Głos z chmury (C1.1a, ADR 0026) schodzi
na koniec kolejki jako opcja. Dokument: `alfa/docs/plan/15-glos-lokalny.md`, zadania C5.1–C5.7.
Moja interpretacja: akceptacja dotyczy kierunku i dodatku do planu; NIE jest zgodą na pobranie silników
(decyzja 1 z dokumentu 15), na znak wodny, ani wyborem źródła próbek głosów — o to pytam osobno.

## 2026-10-11 (wieczór) — Alfa programem docelowym; przeniesienie z Delty; wielu użytkowników

Właściciel po porównaniu z Deltą AI 0.8.35 (`E:\PROGRAMY W CZASIE PRAC\Delta_AI_0.8.35_KOMPLET_2026-09-18`):

- Alfa jest programem docelowym; z Delty przenosimy system rozmowy i mowy oraz WSZYSTKIE brakujące funkcje,
  skrupulatnie (etap P, dokumenty `alfa/docs/plan/16`, `17`; macierz funkcji = zadanie P0).
- Głos Edge „Zofia” tylko jako opcja wyłączona domyślnie, nigdy w sesji prywatnej.
- ADR 0025 (dane biometryczne): „możesz wykonać w pełni, masz zgodę” — przyjęty.
- Biblioteka multimediów nie musi być przenoszona; system odtwarzania i sterowania tak, rozbudowany.
- STAŁA ZGODA na pobieranie plików i modeli do 3 GB bez pytania; powyżej — pytać (od 29.10: 5 GB — wpis niżej).
- Wielu użytkowników po koncie i logowaniu, na Windows, Linux, panelu WWW i Androidzie (etap U, ADR 0027 —
  Proponowany; moja interpretacja: zlecenie kierunku, podpis pod ADR nadal potrzebny, bo zmienia Brokera).
- Agentki mają programować jak asystent programisty: pisać, uruchamiać i budować oprogramowanie z pełnymi
  uprawnieniami w projekcie, rozmawiać ze sobą w trakcie pracy, dobierać model do trudności zadania (P12).
- Praca w pełni automatyczna, wszystko sprawdzane kilka razy; najpierw plan do zatwierdzenia.

## 2026-10-11 20:45 — plan zatwierdzony, start prac

Właściciel: „Zapisz w katalogu plan i zaczynaj prace! pełną siłą!” — plan z `alfa/docs/plan/16–22` (wersja 0.0.29)
zatwierdzony. Kolejność: P1 (rozmowa i mowa) → P2 (modele) → dalej wg dokumentu 16, praca automatyczna.
Dopisane tego dnia grupy: P12 (agentki programistki, przycisk „Pełne uprawnienia” jak w Claude Code/Codex),
P13 (dyski sieciowe i chmurowe, kopie), P14 (obowiązki codzienne — poczta itd.).

---

## Decyzje 2026-10-12 … 2026-10-18

<!-- Dopisane 2026-10-18 (porządek dokumentów, seria 0.0.64) ze źródeł: memory/status.md, memory/handoff.json,
     raporty alfa/docs/plan/26, 27, 28, ADR 0028, dziennik w alfa/docs/plan/03-roadmapa.md, dokument 25 i zasady
     pracy w pamięci asystenta (kopia: alfa/docs/pamiec-projektu/2-zasady-pracy). Przy każdym wpisie: kto zdecydował —
     właściciel czy asystentka w ramach jego polecenia. -->

### [2026-10-12] Pamięć projektu także w repozytorium

**Kontekst:** pamięć leżała tylko poza repozytorium (`memory/`, `~/.claude/projects/…/memory/`, `.remember/`) —
przy przeniesieniu albo kopii projektu zostałaby na starym komputerze.
**Decyzja (właściciel):** „Pamięć projektu i wszystkiego z nim związane mają być też zapisywane w katalogu projektu,
dobrze oznaczone!”.
**Powód:** pamięć ma wędrować razem z projektem.
**Konsekwencje:** `alfa/docs/pamiec-projektu/` (stan, zasady pracy, dziennik sesji) i `scripts/memory-sync.sh` na końcu
każdej sesji (commit `32076843`). Kopia jest lustrem: 17/18.10 na komputerze stacjonarnym pamięć asystenta była pusta
i synchronizacja skasowałaby zasady pracy z repozytorium — od 18.10 na maszynie bez tej pamięci najpierw przywraca się
zasady z `docs/pamiec-projektu/2-zasady-pracy`.
**Źródło:** pamięć asystenta `alfa-pamiec-w-repozytorium.md`; `alfa/docs/pamiec-projektu/README.md`; raport 28, rozdz. 10.
**Status:** Aktywna

### [2026-10-12] Asystentka ma sama testować więcej

**Kontekst:** raporty miały długie listy „nie sprawdziłem”, także rzeczy możliwe do sprawdzenia bez właściciela
(ok. 40 testów sprzętowych `#[ignore]`, polecenia rdzenia w prawdziwej aplikacji, próby funkcji modelowych).
**Decyzja (właściciel):** „zrób tak, żebyś mógł sam więcej testować — na razie dużo rzeczy pomijasz”. Cel właściciela
zapisany w `status.md` przy 0.0.51: pokrycie poleceń rdzenia w prawdziwej aplikacji 70 %.
**Powód:** pomijane sprawdzenia; pierwszy bieg testów sprzętowych od razu znalazł błąd (kwarantanna pobrań a ścieżki 8.3).
**Konsekwencje:** `scripts/verify.mjs` z poziomami `wersja` i `sprzet`, testy sprzętowe, instancja testowa obok Alfy
właściciela, próby obowiązków z oceną; „nie sprawdziłem” tylko z trzech powodów (brak sprzętu, decyzja właściciela,
ocena jego zmysłami). Pokrycie łącza: 38 % (0.0.46) → 71,8 % (0.0.53, bramka 70 %) → 89 % (0.0.63, 235 z 264).
**Źródło:** `alfa/docs/plan/24-samodzielne-testowanie.md`; pamięć `alfa-samodzielne-testowanie.md`; dziennik roadmapy
(0.0.46, 0.0.53); `alfa/docs/plan/00-stan-wykonania.md`.
**Status:** Aktywna

### [2026-10-14] Antigravity: terminal Alfy i pełny most z ostrzeżeniem jako opcja

**Kontekst:** Google AI Pro bez klucza API; regulamin Antigravity (pkt 6) zabrania korzystania z usługi w produktach
innych niż Google; rejestr zgodności miał zakaz „użycia agy z Alfy”.
**Decyzja (właściciel):** „Terminal Alfy i dodatkowo pełny most z ostrzeżeniem jako opcja łatwego włączenia”.
**Powód:** wybór właściciela; ryzyko konta ponosi właściciel — karta mostu to mówi.
**Konsekwencje:** 0.0.52 — logowanie `agy_login` w terminalu Alfy, most `agy` (szara trasa, domyślnie wyłączony,
ostrzeżenie, bez automatycznej zgody, blokady `.agy`/`.gemini`); wpis `agy-antigravity` w rejestrze zgodności
zatwierdzony w nocy 14/15.10 (dokument 25, pkt 3). 15.10 za zgodą właściciela jedno krótkie pytanie na jego koncie
potwierdziło kształt zdarzeń `stream-json` (parser poprawiony w 0.0.56).
**Źródło:** `alfa/docs/plan/25-do-przegladu-po-audycie.md` (§3 i „Rozstrzygnięte”); commity `60e817a7`, `fd7ba08d`;
dziennik roadmapy (0.0.52).
**Status:** Aktywna

### [2026-10-14] Decyzje po audycie (dokument 25) przekazane asystentce

**Kontekst:** po audycie zewnętrznym z 14.10 otwarte zostały: izolacja procesu poleceń powłoki (A-07, część 2), jedna
macierz dowodów (A-15), wpis Antigravity w rejestrze, budżet renderowania Markdown.
**Decyzja (właściciel):** „wykonaj to tak, jak uważasz za najlepsze rozwiązanie dla programu i jego funkcjonalności”.
Rozstrzygnięcia asystentki (0.0.53, noc 14/15.10): (1) izolacja poleceń powłoki integralnością Low, nie AppContainer;
(2) macierz dowodów = generowany `docs/DOWODY.md` z raportów `verify` (od 0.0.51); (3) wpis Antigravity zatwierdzony;
(4) budżet Markdown bez zmian — czyszczenie HTML w kilku wątkach (100 KB w 7,8 ms).
**Powód:** AppContainer odciąłby git, cargo i npm od profilu użytkownika; integralność Low zostawia odczyt, a zapis
poza katalogiem roboczym blokuje sam Windows, nawet gdy analiza polecenia czegoś nie zauważy.
**Konsekwencje:** wyłącznik `tools.shell.low_integrity` (domyślnie włączony); polecenia sieciowe, instalacje i cele
spoza katalogu — integralność Medium i zgoda Brokera jak dotąd; katalog roboczy agentek ma trwałą etykietę Low.
**Źródło:** dokument 25 („Rozstrzygnięte”); raport `26-raport-nocny-2026-10-15.md`, rozdz. 2; commit `9912bda2`.
**Status:** Aktywna

### [2026-10-15] Wersja i budowa co 10 ukończonych punktów

**Decyzja (właściciel):** „zacznij dalsze etapy i nie buduj wersji co jeden punkt tylko co 10”. Zastępuje zasadę
z 11.10 („co pięć dużych ulepszeń”).
**Powód:** budowa powłoki, pełne sprawdzenie i wgranie zjadały czas pracy nad funkcjami (pierwsza budowa od zera po
sprzątaniu dysku: 20–30 min).
**Konsekwencje:** po punkcie — commit, wpis w roadmapie, licznik w `status.md`, bez podnoszenia wersji i budowy; co 10
punktów (albo na polecenie) — jedna wersja z wpisem CHANGELOG, `set-version`, pełny clippy, `verify-app`, wgranie.
Serie: 0.0.60 (seria 1), 0.0.61 (seria 2), 0.0.62 (serie 3 i 4); cel z 18.10 — 0.0.64 i 0.0.65.
**Źródło:** pamięć `alfa-budowanie-aplikacji.md`; raport `27-raport-nocny-2026-10-16.md` (wstęp, rozdz. 1).
**Status:** Aktywna

### [2026-10-15] Wszystko zostaje na dysku E:

**Kontekst:** 14–15.10 dysk E: (NVMe na USB) rozłączał się pod dużym obciążeniem (resety mostka USB, Ntfs 50/140);
diagnostyka 0.0.58 wykazała 200 błędów dysku E: (mostek Realtek RTL9210B).
**Decyzja (właściciel):** wszystko zostaje na E: — nie przenosić `target/` ani niczego innego na C:.
**Powód:** projekt jest na dysku przenośnym (decyzja z 08.10) — dysk przechodzi między komputerami.
**Konsekwencje:** nie proponować przenoszenia na C:; commitować często; po rozłączeniu `git fsck`, przy LNK1207
`cargo clean -p <pakiet>`; ograniczona równoległość cargo (laptop `-j 3`, PC `-j 6`, od awarii 17.10 zawsze
`CARGO_INCREMENTAL=0`).
**Źródło:** pamięć `alfa-srodowisko-lokalne.md` (rozłączenia E:) i `awaria-2026-10-17-obciazenie.md`; dziennik
roadmapy (0.0.58).
**Status:** Aktywna

### [2026-10-15] C6 — Alfa zna komputer i jego narzędzia

**Decyzja (właściciel):** „funkcje, które pozwalają korzystać w pełni z przeglądarek, pisać programy, testować …
system serwerów MCP, by Alfa mogła tworzyć i dostosowywać się do warunków na maszynie” oraz „system diagnostyki
komputera, bardzo dokładny, z zapisem parametrów i testami systemów”.
**Powód:** polecenie właściciela — nowa grupa zadań.
**Konsekwencje:** grupa C6 w roadmapie: C6.1–C6.3 (0.0.58: programy na maszynie, serwery MCP użytkownika, diagnostyka
`pc-diag`), C6.4–C6.5 (0.0.59, zbudowane w 0.0.60: przeglądarka Alfy, `tools-dev`); od 17.10 — C6.6.
**Źródło:** `alfa/docs/plan/03-roadmapa.md` — C6.
**Status:** Aktywna

### [2026-10-15] Kopia bezpieczeństwa projektu na dysku F: (katalog Alfa)

**Decyzja (właściciel):** noc 15/16.10 — „kopia bezpieczeństwa na dysku F: gotowa na 5:00”; noc 17/18.10 — „pracuj
dalej do 3:30, potem kopia bezpieczeństwa na dysk F: (katalog Alfa)”.
**Powód:** zabezpieczenie jedynej kopii roboczej na zawodnym dysku E:.
**Konsekwencje:** `scripts/backup.sh` (domyślnie `F:\Alfa\kopia-<data>`: paczka git, katalog roboczy bez kompilacji,
pamięć, `SHA256SUMS.txt`, `ODTWORZENIE.md`). Kopie sprawdzone sumami: z 16.10 (raport 27) oraz `kopia-2026-10-17_2115`,
`kopia-2026-10-18_0054` i `kopia-2026-10-18_0330` (stan `678e7b2a`) — wszystkie na dysku F: komputera stacjonarnego.
**Źródło:** raport 27, rozdz. 6; raport 28 (wstęp, rozdz. 6); `status.md`.
**Status:** Zastąpiona 2026-10-18 — kolejna kopia dopiero po 0.0.65, na nowym dysku (wpis niżej)

### [2026-10-17] C6.6 — Alfa ma działać na każdej karcie graficznej

**Kontekst:** dysk E: przeniesiony na komputer stacjonarny (Ryzen 7 5700X3D, RX 9070 XT 16 GB, 32 GB RAM); silniki
pobrane na laptopie były w wariancie CUDA, a Whisper na AMD liczył na procesorze.
**Decyzja (właściciel):** „Alfa ma działać na kartach AMD i Nvidia i iGPU różnego typu, więc musi być to dopracowane”,
potem „pobierz to, co potrzeba, i dodaj do programu, i zapisz odpowiednio wszystko w dokumentacji” i „najwyższa
precyzja”.
**Powód:** polecenie właściciela (program ma działać na każdej karcie, nie tylko NVIDIA).
**Konsekwencje:** zadanie C6.6 i seria 0.0.63: wykrywanie od sterownika (DXCore), klasy niezależne od producenta, klasa
„karta zintegrowana”, APU z rezerwą ≥ 6 GB liczone jak karta, pomiar pamięci kart licznikami Windows; mowa i głos —
dwa kolejne wpisy. Zostaje: próba na laptopie z kartą zintegrowaną i na APU AMD, osobny budżet pamięci na kartę.
**Źródło:** raport `28-raport-nocny-2026-10-18.md` (wstęp, rozdz. 1); roadmapa C6.6; pamięć
`maszyna-stacjonarna-2026-10-17.md`.
**Status:** Aktywna (C6.6 w toku)

### [2026-10-17] Mowa na AMD/Intel: Qwen3-ASR przez `llama-server` Vulkan

**Kontekst:** oficjalne wydania whisper.cpp nie mają kompilacji Vulkan dla Windows (sprawdzone do v1.9.5) — na AMD/Intel
Whisper liczył na procesorze (ok. 54 s na zdanie na laptopie, Whisper turbo).
**Opcje:** własna kompilacja whisper.cpp z Vulkanem (pliki spoza oficjalnych wydań) vs Qwen3-ASR przez istniejący
`llama-server` Vulkan.
**Decyzja (asystentka w ramach polecenia C6.6; ADR 0028, pkt 1):** Qwen3-ASR przez `llama-server` Vulkan w pakietach
5 i 4 na kartach AMD/Intel (także samej zintegrowanej); Whisper zostaje zapasem.
**Powód:** bez nowego silnika i bez kompilowania whisper.cpp; zmierzone na RX 9070 XT: 0,16–0,23 s na zdanie,
2878 MB pamięci karty.
**Konsekwencje:** na karcie poniżej 12 GB — reguła z 18.10 (wpis niżej); karty Intel niesprawdzone.
**Źródło:** ADR 0028 (decyzja, pkt 1); `handoff.json` → `decisions_this_session`; raport 28.
**Status:** Aktywna w kodzie (0.0.63); ADR 0028 Proponowany — czeka na podpis właściciela

### [2026-10-17] DirectML tylko dla głosu Supertonic, na karcie z własną pamięcią ≥ 12 GB

**Kontekst:** Supertonic i Smart Turn liczą na procesorze przez ONNX Runtime CPU; DirectML jest w trybie podtrzymania
(ADR 0004 odrzucił go jako wspólny backend).
**Decyzja (asystentka na polecenie właściciela „pobierz to, co potrzeba, i dodaj do programu”; ADR 0028, pkt 2):**
DirectML opcjonalnie i wąsko — tylko Supertonic w osobnym procesie (`alfa-supertonic --dml`), z jawnym powrotem na
procesor, tylko przy karcie z własną pamięcią ≥ 12 GB; Smart Turn zostaje na procesorze. Pozycje katalogu z NuGet
(sumy zgodne): `onnxruntime-directml` 1.23.0 (MIT) i `directml` 1.15.4 (licencja Microsoftu).
**Powód:** zmierzone na RX 9070 XT: 0,58 s zamiast 0,95 s na zdanie, 538 MB pamięci karty; Supertonic nie ma dzierżawy
w `model-residency`, stąd próg 12 GB (na karcie 8 GB model 4.5B i STT zajmują 7380 z 7408 MB budżetu).
**Konsekwencje:** licencja DirectML (§2a) dopuszcza zbieranie danych diagnostycznych przez Microsoft — pozycja
`directml` ma `confirmed: false`, użytkownik pobiera ją z NuGet, nie jest dołączana do instalatora. Na NVIDIA i Intel
≥ 12 GB niemierzone; na karcie zintegrowanej obok osobnej wyłączone.
**Źródło:** ADR 0028 (decyzja pkt 2, konsekwencje, „Do decyzji właściciela”); raport 28, rozdz. 5.
**Status:** Proponowana — czeka na podpis ADR 0028, potwierdzenie licencji DirectML i odpowiedź, czy mierzyć DirectML
na karcie zintegrowanej obok osobnej (tabela „Decyzje właściciela, które blokują zadania” w roadmapie)

### [2026-10-17] Porządki i pobrania na komputerze stacjonarnym

**Decyzja (właściciel):** zgoda na pobranie brakujących pakietów `node_modules` z lockfile; zgoda na usunięcie tylko
starej kompilacji powłoki (5,5 GB) — `PLAN/`, `alfa-glos-proba` i kopie na F: zostają.
**Interpretacja asystentki (nie nowe słowa właściciela):** artefakty kompilacji w `target/` to odtwarzalna pamięć
podręczna; stała zgoda na pobrania do 3 GB z 11.10 obejmuje łatki z crates.io (np. `ammonia` 4.2.4) i przeglądarkę
testową Playwrighta.
**Powód:** dysk E: zapełnił się do 0 GB; bez `node_modules` nie działały testy interfejsu.
**Konsekwencje:** usunięte ok. 97 GB, a wieczorem kolejne 130 GB odtwarzalnych artefaktów; resztki starego
`node_modules` (`stare-node_modules-acl-2026-10-17`) usunie tylko właściciel z prawami administratora.
**Źródło:** `handoff.json` → `decisions_this_session`; raport 28, rozdz. 5, 6 i 10; pamięć
`maszyna-stacjonarna-2026-10-17.md`.
**Status:** Aktywna

### [2026-10-18] Reguła 12 GB dla Qwen3-ASR na Vulkanie zostaje (po ósmym przeglądzie)

**Kontekst:** całościowy przegląd zmian od 0.0.62 wykazał, że na karcie AMD/Intel 6–11 GB Qwen3-ASR (ok. 2,9 GB)
i model rozmowy mogą się razem nie zmieścić.
**Opcje:** (A) Qwen3-ASR na procesorze na każdej karcie Vulkan poniżej 12 GB; (B) rezerwa 3 GB na mowę w budżecie
modelu rozmowy (commit `ba880bcf`).
**Decyzja (asystentka, po ósmym przeglądzie w świeżym kontekście):** A — zostaje reguła 12 GB
(`VULKAN_QWEN_CARD_MIN_MB`), karta w całości dla modelu rozmowy.
**Powód:** rezerwa 3000 MB jest mniejsza niż próg Qwen3-ASR (3200 MB wolnej pamięci), liczy się raz przy starcie
programu i nie widzi ustawienia „tylko Whisper”; dzielenie karty 6–8 GB bez osobnego budżetu pamięci na kartę jest za
kruche, a takiej karty nie było do pomiaru.
**Konsekwencje:** na karcie Vulkan poniżej 12 GB mowa liczy na procesorze (ok. 2,4 s na zdanie — ponad 20 razy szybciej
niż Whisper na procesorze w 0.0.62). Ograniczenie: reguła działa także wtedy, gdy model rozmowy nie korzysta z karty
(klasa „nieznana”, rozmowa przez API) — do zadania „osobny budżet pamięci na kartę” (C6.6).
**Źródło:** raport 28, rozdz. 12–13; commit `96588e5b`; ADR 0028, pkt 1; SPEC `voice-stt`.
**Status:** Aktywna (0.0.63); jako część ADR 0028 czeka na podpis

### [2026-10-18] Wersja 0.0.63 niewgrana — wgranie należy do właściciela

**Decyzja:** 0.0.63 nie jest wgrana do `Alfa-uruchomiona` (tam 0.0.62). Wersja do wgrania (`status.md`: „decyzja
właściciela”) = `target/shell/debug/alfa-desktop.exe` zbudowana z `21897bf0` (kod jak w `96588e5b`, inny tylko tekst
„Co nowego”). Samo wgranie — albo od razu 0.0.64/0.0.65 — po lekturze raportu 28.
**Powód:** ADR 0028 i licencja DirectML czekają na właściciela, a pakiety z głosem Supertonic na karcie ≥ 12 GB (jak
RX 9070 XT) zawierają DirectML.
**Konsekwencje:** każda budowa powłoki w `target/shell` nadpisze kandydata 0.0.63.
**Źródło:** raport 28 (wstęp, rozdz. 5 pkt 4, rozdz. 10 i 14); `status.md` (18.10); `handoff.json`.
**Status:** Oczekuje na decyzję właściciela

### [2026-10-18] Cel: wszystko porządnie do 0.0.65; kopia po 0.0.65 na nowym dysku

**Kontekst:** dysk E: wrócił z komputera stacjonarnego na laptop. F: na laptopie to inny dysk („Dane”, 1,9 TB,
zapełniony plikami testu pojemności CAPACITYTESTER) — kopie `F:\Alfa\kopia-…` są na dysku F: komputera stacjonarnego
i z laptopa nieosiągalne.
**Decyzja (właściciel, polecenie /goal):** wykonać wszystko porządnie do wersji 0.0.65 (0.0.64 i 0.0.65 = serie po
10 punktów), z większą liczbą agentów; kopia bezpieczeństwa dopiero po 0.0.65, na nowym dysku, który podłączy
właściciel.
**Powód:** na laptopie nie ma dysku na kopie (wniosek z kontekstu — właściciel podłączy nowy).
**Konsekwencje:** `scripts/backup.sh` z domyślnym celem `/f/Alfa` NIE uruchamiać na laptopie; po 0.0.65 kopia z jawnym
celem i sprawdzeniem sum (w serii: `backup.sh` z celem jako parametrem i sprawdzeniem miejsca). Plan serii w
`handoff.json` → `next_session`.
**Źródło:** `status.md` (18.10); `handoff.json` (`session_goal`, `next_session`, `blocking_issues`); pamięć
`maszyna-stacjonarna-2026-10-17.md`.
**Status:** Aktywna

### [2026-10-18] Praca na zmianę na dwóch komputerach — reguły maszyny

**Kontekst:** dysk E: przechodzi między laptopem (i7-13620H, 16 GB RAM, RTX 4050 6 GB + Intel UHD Graphics; są Edge,
`jq`, `gh`, Claude Code i dane Alfy) a komputerem stacjonarnym (Ryzen 7 5700X3D, RX 9070 XT 16 GB, 32 GB RAM; bez Edge,
`jq`, `gh`, Claude Code i danych Alfy). Pamięć i raporty pisane raz na jednej, raz na drugiej maszynie mieszały ich
reguły.
**Decyzja (zasada pracy asystentki po przeniesieniu dysku przez właściciela):** na początku sesji sprawdzić maszynę
(procesor, karty) i stosować jej reguły — laptop `-j 3`, komputer stacjonarny `CARGO_BUILD_JOBS=6`, zawsze
`CARGO_INCREMENTAL=0`; nie porównywać czasów między maszynami; przed `verify sprzet` ustalić z właścicielem, czy maszyna
jest docelowa; dokumenty stanu opisują obie maszyny (`alfa/docs/plan/00-stan-wykonania.md`).
**Powód:** reguły jednej maszyny na drugiej są błędne albo groźne (np. `-j 6` przy 16 GB RAM); awaria komputera
stacjonarnego 17.10 o 20:31 (brak pamięci i resety USB) przy pełnych testach.
**Źródło:** pamięć `maszyna-stacjonarna-2026-10-17.md` i `awaria-2026-10-17-obciazenie.md`; `status.md` (18.10).
**Status:** Aktywna

### [2026-10-18] Wtyczka „remember” wyłączona

**Kontekst:** hak po każdym poleceniu (`post-tool-hook`) liczył linie zapisu sesji (ok. 350 MB) — dziesiątki procesów
po 500 s procesora spowalniały laptop i każdego agenta.
**Decyzja (właściciel):** wtyczka wyłączona; zawieszone procesy zatrzymane.
**Powód:** spowolnienie całej maszyny.
**Konsekwencje:** na początku sesji na laptopie sprawdzić, że nie działają procesy `post-tool-hook.sh`.
**Źródło:** `status.md` (18.10); `handoff.json` → `next_session`.
**Status:** Aktywna

### [2026-10-29] Stała zgoda na pobrania podniesiona do 5 GB

**Kontekst:** próba C2.3 (`live_catalog`) pobiera Bielika 4.5B (4,8 GB) z silnikami — ponad dawną granicę 3 GB.
**Decyzja (właściciel):** „zmień limit pobierania do 5GB”, a dla tej próby: „wykonaj i tak pobieranie, jeśli to ma
być lepsze i wydajniejsze”; pobierać na E: (po sprzątaniu starych kompilacji), nie na C:.
**Powód:** pytania o pobrania przerywają pracę; dysk E: ma miejsce po usunięciu starych kompilacji.
**Konsekwencje:** do 5 GB bez pytania, powyżej pytać z rzeczywistym rozmiarem (z zależnościami); katalog pobrań
poza repozytorium na E:, po próbie posprzątany. 29.10 wieczorem usunięto stare kompilacje (`alfa` 166 → 23 GB).
**Źródło:** rozmowa 29.10; pamięć `alfa-zgoda-na-pobrania.md`.
**Status:** Aktywna

### [2026-11-01] Testy prawdziwej aplikacji mogą uruchamiać `claude auth status`

**Kontekst:** test `z-terminal` po „Gotowe” sprawdza stan logowania mostu CLI; rdzeń uruchamia wtedy
`claude auth status --json` na komputerze właściciela (instancja testowa widzi prawdziwy plan). Raport 47 §5.
**Opcje:** A — zostawić (test sprawdza prawdziwe zachowanie) vs B — podstawić testom atrapę polecenia.
**Decyzja (właściciel):** A — „testy mogą uruchamiać claude auth status na tym komputerze”.
**Konsekwencje:** `z-terminal` bez atrapy. Zgoda dotyczy tylko polecenia stanu logowania z testów prawdziwej
aplikacji; Alfa nadal nie czyta tokenów (`~/.claude`, `~/.codex`), a sesje AI nie uruchamiają mostów CLI same.
**Źródło:** rozmowa 1.11 rano.
**Status:** Aktywna

### [2026-11-06] Etap Z scalony; Alt+M; sędzia przy małym modelu; PIN w pliku Sędziego

**Kontekst:** raport 53 §4 i raport 52 §4 — pytania o scalenie gałęzi `do-przegladu-tryby-zgod`, sędziego przy
małym modelu lokalnym, zmianę trybów w rozmowie „do końca rozmowy”, `spawn_blocking` i przechowywanie PIN-u.
**Decyzja (właściciel 6.11 rano):** „scalić gałąź tak”; sędzia zatwierdzający sam tylko proste, bezpieczne akcje —
„ok”; w rozmowie „do końca rozmowy” tryby zmieniane w każdej chwili jak w Claude Code i Codex; „resztę wykonaj tak,
by było ok”.
**Wykonanie:** Alt+M — następny tryb (jak Shift+Tab w Claude Code; bez Pełnego dostępu), także w rozmowie
„do końca rozmowy”; przegląd drzewa w `spawn_blocking`; skrót PIN-u (Argon2id) zostaje w prywatnym pliku Sędziego —
Menedżer poświadczeń użytkownika może zmieniać samo konto dziecka, katalog usługi nie; profil Dziecko bez usługi
Windows dopuszczony z ostrzeżeniem na karcie (ochrona umowna), usługa zalecana.
**Źródło:** rozmowa 6.11 rano; raport 53.
**Status:** Aktywna
