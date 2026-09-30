# THREAT_MODEL.md — model zagrożeń Alfy

Źródło: `docs/PLAN.md` §8.0 (model zagrożeń), §8 (bezpieczeństwo i uprawnienia), §1.3 (zgodność tras abonamentowych), §6.10 (bezpieczeństwo głosu), §17 (ryzyka). Szkic powstaje w F0 (sesja #1), jest uzupełniany w F3 (Broker) i stanowi listę kontrolną pentestu w F9.

## 1. Założenia

- Użytek osobisty: jeden użytkownik (właściciel), jego maszyny, brak dystrybucji. Nie chronimy przed właścicielem; chronimy właściciela przed skutkami błędu modelu, wstrzykniętego polecenia i złośliwej treści.
- Agentki mają domyślnie **bardzo wysoką autonomię (L3)**, opcjonalnie **L4 „Maks"**. Model zagrożeń musi działać także na L4 — dlatego kluczowe kontrole są w Jądrze, poza zasięgiem agentek.
- Kod programu powstaje w całości przez modele AI i jest automatycznie mergowany → sam proces budowy jest w zakresie modelu zagrożeń (runner, łańcuch dostaw).
- Uczciwie: na L4 bez izolacji audyt jest „best effort"; DPAPI chroni sekrety przed kradzieżą offline, nie przed procesem tego samego użytkownika.

## 2. Aktywa

| Aktywo | Gdzie | Co grozi |
|---|---|---|
| Pliki użytkownika i sekrety (dokumenty, klucze SSH, profile) | dysk, `%USERPROFILE%` | usunięcie, wyciek, szyfrowanie |
| Sesje przeglądarki i ciasteczka | profile przeglądarek | przejęcie kont |
| Klucze API i konta dostawców | Windows Credential Manager | wyciek, koszty |
| Poświadczenia CLI (`~/.claude`, `~/.codex`) | katalogi CLI | naruszenie regulaminów, przejęcie planu |
| Pamięć długoterminowa agentek | SQLite szyfrowane | zatrucie, wyciek, trwała manipulacja zachowaniem |
| Integralność Jądra, polityk i audytu | Broker, pliki audytu | wyłączenie zabezpieczeń, zatarcie śladów |
| Prywatność audio i ekranu | mikrofon, zrzuty, OCR, logi GUI | podsłuch, wyciek do chmury |
| Integralność kodu i aktualizacji | repo, runner, `updater` | złośliwy kod w buildzie |
| Pieniądze (koszty API) | `cost-meter` | wyczerpanie budżetu |

## 3. Aktorzy

| Aktor | Wektor wejścia | Zdolności | Motyw / charakter |
|---|---|---|---|
| Niezaufana treść tekstowa | strony WWW, e-mail, pliki, wyniki narzędzi, treść ekranu (OCR/UIA) | prompt injection, fałszywe instrukcje „od użytkownika" | złośliwy autor treści |
| **Niezaufany dźwięk** | TV, YouTube, podcast, rozmowa obok, syntetyczny głos właściciela | komendy głosowe, injection przez STT | przypadek lub atak celowy |
| Złośliwy serwer / opis narzędzia MCP | konfiguracja MCP, import z Kreatora | zatruty opis narzędzia, zmiana opisu po instalacji, eksfiltracja przez argumenty | dostawca narzędzia |
| Zatruta wtyczka / umiejętność | Wasm, playbook, wpis katalogu dostawców | wykonanie kodu w sandboxie, próba wyjścia | autor wtyczki, Ulepszacz |
| Inny proces użytkownika | ten sam SID | odczyt pamięci, SendInput do okien, DPAPI | malware na maszynie |
| Halucynujący / błądzący model | każdy adapter | błędne polecenia destrukcyjne, pętle, wyczerpanie budżetu | brak złej woli, ale skutek identyczny |
| Atak sieciowy | DNS rebinding, MITM na `baseURL` adaptera, złośliwy endpoint | przejęcie lokalnych serwerów, podmiana odpowiedzi | zewnętrzny |
| Samo-modyfikujący się Ulepszacz | pierścienie R0–R2 | Goodhart, rozszerzenie własnych uprawnień, zmiana progów | wewnętrzny, systemowy |
| Most CLI (Claude Code, Codex) jako „opaque worker" | proces potomny | działa z uprawnieniami CLI, własne narzędzia fs/shell | zaufany warunkowo |
| Kod wygenerowany przez AI w repo | PR z automerge, self-hosted runner | wykonanie na maszynie właściciela | błąd lub zatrucie kontekstu sesji deweloperskiej |

## 4. Granice zaufania

```
┌──────────────── Niezaufane ────────────────┐
│ WWW · e-mail · pliki · ekran · dźwięk z TV │
│ serwery MCP · wtyczki · endpointy API      │
└──────────────┬─────────────────────────────┘
               │ taint: sesja oznaczana `tainted`
┌──────────────▼──────── Procesy agentek (konto użytkownika) ───────────────┐
│ Jądro Alfa Core (magistrala, rejestr, config, core-log)                   │
│ agent-runtime · router · memory · voice-* · tools-* (restricted token /   │
│ low-integrity / AppContainer dla ≤ L3) · Wasm (wasmtime, bez importów)    │
│ ▲ UI (WebView2, minimalne capabilities, ścisłe CSP, markdown z Rust)       │
└──────┬──────────────────────────────────────────────────────┬─────────────┘
       │ named pipe z ACL na SID, tokeny zdolności             │ stdio / pipe
┌──────▼──────────── Zaufane, poza zasięgiem agentek ─────────┐  ┌──────▼──────────┐
│ BROKER (usługa, osobne konto Windows): tokeny, polityki      │  │ Mosty CLI        │
│ Jądra, jedyny writer audytu, kill-switch, deny-listy, UAC    │  │ (opaque worker,  │
│ BROKER-UI (Twoja sesja, wyższy poziom integralności — UIPI)  │  │ worktree, MCP    │
│ WATCHDOG (osobny proces, Job Objects, safe-mode, rollback)   │  │ tylko Windows)   │
│ Helper uiAccess (Program Files, podpisany, zakaz gui.control)│  └──────────────────┘
└──────────────────────────▲───────────────────────────────────┘
                           │ fizyczne wejście (klik/klawisz), opcjonalnie Windows Hello
                    ┌──────┴──────┐
                    │  WŁAŚCICIEL │
                    └─────────────┘
```

Zasady przekraczania granic:
- Z niezaufanego do agentek: treść jest **danymi**, nigdy instrukcjami; sesja dostaje flagę `tainted`.
- Z agentek do Brokera: wyłącznie żądania tokenów zdolności (`fs.read/write(zakres)`, `shell.exec`, `gui.control(app)`, `net.egress(host)`, `secrets.read`, `system.admin`) z TTL; **potomek ≤ rodzic**; reguły mogą tylko zawężać.
- Z Brokera do właściciela: zatwierdzenia wyłącznie w Broker-UI, wyłącznie fizycznym wejściem; nigdy w toaście, nigdy w WebView z treścią LLM.
- Agentki nie mają drogi do Brokera innej niż pipe: zakaz `gui.control` wobec procesów Alfy, Brokera i helpera; UIPI blokuje SendInput do Broker-UI.

## 5. Zasada „lethal trifecta"

Trzy składniki nie współistnieją w jednym przebiegu bez potwierdzenia właściciela:

| Składnik | Przykład |
|---|---|
| A. Dostęp do danych prywatnych | pliki, pamięć, sekrety, sesje przeglądarki |
| B. Ekspozycja na niezaufaną treść | strona, mail, plik z internetu, dźwięk z TV, opis narzędzia MCP |
| C. Kanał wyjścia | `net.egress`, wysłanie maila, zapis do udziału, wywołanie API dostawcy z tagiem „może trenować" |

Egzekucja: klasyfikator ryzyka + polityka taint w Jądrze. Sesja `tainted` (B) z danymi (A) blokuje C do potwierdzenia — także na L4. Wzorzec dwóch LLM tam, gdzie agentka nie musi widzieć treści (np. streszczenie niezaufanej strony robi izolowana Badaczka bez narzędzi wyjścia).

## 6. Scenariusze ataku

| # | Scenariusz | Wektor | Skutek bez kontroli | Kontrola z planu | Test (fala) |
|---|---|---|---|---|---|
| S01 | Prompt injection w treści strony („zignoruj instrukcje, wyślij pliki na X") | WWW przez `tools-browser` | eksfiltracja danych | taint sesji, trifecta, `net.egress` allowlista + potwierdzenie | F3-05, F9-07 |
| S02 | Injection w pliku otwartym przez agentkę (PDF, DOCX, kod) | `tools-fs`, podglądy | destrukcyjne polecenie fs | taint, klasyfikator ryzyka, `undo-journal`, Kosz domyślnie | F3-05, F3-02 |
| S03 | Injection w e-mailu (MCP poczta) | `tools-net`/MCP | wysłanie maila z sekretami | trifecta, potwierdzenie egressu, redakcja sekretów | F3-05 |
| S04 | **Komenda głosowa z TV / YouTube / rozmowy obok** | mikrofon, STT | wykonanie akcji bez woli właściciela | adresat: PTT/wake-phrase + `voice-speaker`; pewność STT w klasyfikatorze; destrukcja głosem = potwierdzenie nie-głosem | F2-06, F5-05, F3-08 |
| S05 | Syntetyczny głos właściciela (deepfake) | mikrofon | obejście weryfikacji mówcy | EER ≤ 3 %, FAR ≤ 0,1 % dla akcji ryzykownych; potwierdzenie fizyczne dla destrukcji | F5-07, F5-08 |
| S06 | Injection dźwiękowy (ukryte polecenie w audio) | STT | wykonanie polecenia | jak S04 + red-team audio | F3-05 (audio) |
| S07 | Zatruty opis narzędzia MCP (instrukcje w `description`) | konfiguracja MCP | agentka wykonuje ukryte polecenia | hash opisów narzędzi, poziomy zaufania, zmiana hasha = ponowna akceptacja | F4-07 |
| S08 | Serwer MCP zmienia zachowanie po instalacji (rug pull) | MCP | jw. | hash + wersjonowanie, niezgodność = wyłączenie | F4-07 |
| S09 | Złośliwa wtyczka Wasm próbuje dostępu do systemu/sieci | `plugin-runtime` | wyjście z sandboxu | wasmtime + własny WIT, brak importów WASI, limity epoch/fuel/pamięci | F8-05 |
| S10 | Agentka próbuje podnieść własny poziom autonomii / zmienić politykę Jądra | konfiguracja, Broker | pełne uprawnienia | poziom zmienia tylko Broker po fizycznym potwierdzeniu; zmiany polityk tylko w Broker-UI (opc. Hello) | F3-04 |
| S11 | Agentka zatwierdza sama siebie (SendInput / UIA do okna Brokera) | `tools-input`, `tools-uia` | obejście zatwierdzeń | Broker-UI na wyższym poziomie integralności (UIPI), zakaz `gui.control` wobec Alfy, wejście niewstrzyknięte | F0-16, F3-04, F6-06 |
| S12 | XSS w renderowanym markdownie → wykonanie w WebView → IPC do Rust | odpowiedź LLM | RCE | markdown do HTML w Rust (pulldown-cmark + ammonia), bez surowego HTML, ścisłe CSP, Trusted Types, minimalne capabilities per okno, iframe bez IPC dla artefaktów | F1-07 (CSP), F9-01 |
| S13 | Otwarty port CDP WebView2 w produkcji | sieć lokalna | zdalne sterowanie UI | CDP tylko w buildzie testowym; test CI | F3-13 |
| S14 | DNS rebinding / złośliwy `baseURL` adaptera generycznego | sieć | przejęcie lokalnych usług, podmiana odpowiedzi | egress-allowlista, serwer MCP tylko stdio/named pipe z ACL, bez nasłuchu TCP; walidacja hostów | F4-07 |
| S15 | Kradzież tokenów CLI / ciasteczek przeglądarki przez agentkę | `tools-fs`, `tools-browser` | naruszenie regulaminów, przejęcie kont | deny-lista ścieżek (`~/.claude`, `~/.codex`, profile przeglądarek, Credential Manager) w `fs.*`/shellu; monitor ETW | F0-06, F4-04, F6-05 |
| S16 | Agentka „używa" webowego UI dostawcy (claude.ai, chatgpt.com) | `tools-browser`, `gui.control` | naruszenie regulaminów | deny-lista domen/aplikacji dostawców w GUI/przeglądarce | F6-05 |
| S17 | Most CLI uruchomiony z harmonogramu, poza wolą właściciela | `triggers` | naruszenie zasad użycia planu, koszty | mosty tylko na żądanie; harmonogramy mostów jawnie włączane z limitami; Ulepszacz nie używa mostów | F5-04 |
| S18 | Most CLI omija nasze zatwierdzenia własnymi narzędziami | proces CLI | akcje bez kontroli | „opaque worker": worktree/kopia, `--permission-prompt-tool` / approvals → Broker-UI, przez MCP tylko narzędzia Windows (bez fs/shell), snapshot przed/po | F0-05, F4-03 |
| S19 | Zatrucie pamięci długoterminowej przez niezaufaną treść | `memory` | trwała manipulacja zachowaniem | proweniencja wpisów; wpis z niezaufanej treści nie awansuje do „globalnej"; auto-`remember` z niezaufanego wyłączony | F7-04 |
| S20 | Ulepszacz „optymalizuje" metrykę kosztem bezpieczeństwa (Goodhart) | R0–R2 | rozluźnienie kontroli | bramka ewaluacyjna w Jądrze, ukryty holdout, N ≥ 5, tylko zmiany zawężające; zakaz zmian tagów, budżetów, uprawnień, allowlisty, progów | F8-02, F8-03 |
| S21 | Wyłączenie lub sfałszowanie audytu | dowolny proces agentki | zatarcie śladów | Broker jedynym writerem, pliki append-only przez ACL, łańcuch hashy z kotwicą poza zasięgiem agentek | F3-09 |
| S22 | Pętla agentki / wyczerpanie budżetu API | halucynacja | koszty, zawieszenie | detektor pętli, budżety tokenów/czasu/PLN, limit miesięczny, circuit breaker | F3-07, F5-03 |
| S23 | Złośliwy crate / model / aktualizacja w łańcuchu dostaw | `Cargo.lock`, pobieranie modeli, `updater` | wykonanie obcego kodu | `cargo deny/vet`, lockfile, SBOM; modele tylko safetensors/ONNX/GGUF z hashami; aktualizacje podpisane minisign | F0-01, F3-10 |
| S24 | Kod pisany przez AI i automergowany wykonuje się na runnerze właściciela | self-hosted runner | dostęp do profilu właściciela | runner na osobnym koncie Windows bez dostępu do profilu, repo prywatne, bez PR z forków, Jądro/Broker przez człowieka | proces (§4.2) |
| S25 | Wyciek audio/ekranu do chmury wbrew tagowi prywatności | `router`, `voice-*` chmurowe | prywatność | macierz „dane → dostawca" egzekwowana tagiem sesji (także mosty i chmurowy głos); trasy CN/„może trenować" zablokowane dla sesji prywatnych; ekran „co poszło do chmury" | F1-13, F4-05 |
| S26 | Zrzut ekranu / OCR przechwytuje hasło lub okno bankowości | `tools-vision` | wyciek | pola `IsPassword` i deny-lista aplikacji/okien/URL wykluczone ze zrzutów/OCR; retencja 7 dni, szyfrowanie | F6-05 |
| S27 | Otwarta usługa admina dostępna dla każdego procesu | Broker | eskalacja do SYSTEM | elewacja tylko na żądanie przez UAC; opt-in usługa z krótką allowlistą, weryfikacją Authenticode+SID i zgodą per eskalacja | F3-04, F6-07 |
| S28 | Inny proces użytkownika czyta sekrety z pamięci procesu Alfy / DPAPI | malware lokalne | wyciek kluczy | poza modelem (ten sam SID) — uczciwie: DPAPI nie chroni; minimalizacja czasu życia sekretów w pamięci, redakcja w logach | F9-01 (opis) |

## 7. Twarde blokady Jądra (obowiązują na każdym poziomie, także L4)

| Blokada | Egzekwuje |
|---|---|
| Wyłączenie audytu, watchdoga, kill-switcha | Broker (odmowa, brak takiej zdolności) |
| Formatowanie dysku systemowego, usuwanie `%SystemRoot%`, modyfikacja bootloadera | Broker + deny-lista shella |
| Zmiana polityk Jądra, tagów prywatności, egress-allowlisty, budżetów, progów bramki ewaluacyjnej | tylko Broker-UI po fizycznym potwierdzeniu (opc. Hello) |
| Podniesienie własnego poziomu autonomii przez agentkę | Broker |
| `gui.control` wobec procesów Alfy, Brokera, Broker-UI, helpera | Broker + UIPI |
| Odczyt poświadczeń CLI, profili przeglądarek, Credential Manager przez narzędzia agentek | deny-lista `fs.*`/shell |
| Automatyzacja webowych UI dostawców, wyciąganie ciasteczek/tokenów | deny-lista domen/aplikacji |
| Destrukcyjna akcja zlecona głosem bez potwierdzenia nie-głosem | Broker (wyjątek tylko ręcznie w Broker-UI) |
| Egress z sesji `tainted` przy dostępie do danych prywatnych bez potwierdzenia | polityka taint |
| Uruchomienie mostu CLI z wyzwalacza bez jawnego włączenia | `triggers` + Broker |
| Zmiana Jądra, Brokera lub polityk przez Ulepszacza/Diagnostę (R3 poza v1) | bramka ewaluacyjna, brak zdolności |

## 8. Poziomy autonomii — co zostaje na L4

| Poziom | Pyta o | Co nadal działa niezależnie od poziomu |
|---|---|---|
| L0 Podgląd | wszystko (nic nie zmienia) | |
| L1 Pytaj o wszystko | każdą zmianę | |
| L2 Pytaj o ryzykowne | usuwanie, egress, instalacja | |
| **L3 Bardzo wysoka (domyślny)** | nieodwracalne poza zakresem; działania na podstawie niezaufanej treści | |
| **L4 Maks** | nic poza twardymi blokadami (§7) i destrukcją zleconą głosem | **kill-switch < 200 ms, dziennik cofania, audyt (best effort), tokeny zdolności z TTL, potomek ≤ rodzic, deny-listy §1.3, tagi prywatności, budżety kosztów, klasyfikator ryzyka i taint dla egressu** |

Podniesienie do L4: jeden przełącznik w Ustawieniach (globalnie / sesja / agentka / na czas), potwierdzany fizycznym wejściem w Broker-UI, opcjonalnie Windows Hello. Agentka i treść ze strony nie mogą go wykonać. „Zawsze zezwalaj w tym zakresie" nie eskaluje do L4.

## 9. Zgodność tras abonamentowych jako wymaganie bezpieczeństwa

Naruszenie regulaminu dostawcy = utrata konta właściciela (aktywo: plany Claude/ChatGPT). Dlatego zasady z §1.3 planu są traktowane jak kontrole bezpieczeństwa, nie jak polityka:

| Wymaganie | Kontrola techniczna | Test |
|---|---|---|
| Most = sterowanie oficjalnym, niezmodyfikowanym CLI, do którego loguje się właściciel | `agent-backends` uruchamia binarkę CLI w trybie nieinteraktywnym/SDK/app-server; przypięte wersje CLI (nieznana wersja wyłącza trasę) | F4-08 |
| Kod Alfy nigdy nie czyta ani nie przechowuje tokenów CLI | deny-lista ścieżek; monitor ETW: 0 odczytów | F0-06, F4-04 |
| Agentka nie używa webowego UI dostawców | deny-lista domen/aplikacji w `tools-browser`/`gui.control` | F6-05 |
| Uruchamianie mostów tylko na żądanie właściciela | harmonogramy mostów wyłączone domyślnie; Ulepszacz nie ma dostępu | F5-04 |
| Rejestr zgodności świeży | `compliance-registry.json`: status, data, cytat, archiwalna kopia regulaminu; nieświeży = trasa „szara"; wyłącznik | F1-13, F4-05, F4-06 |
| Trasy zabronione (Qwen Coding Plan, GLM/ZCode przez plan) nigdy nie są budowane | brak adaptera; modele tylko przez API | przegląd kodu |

## 10. Próg red-team (bramka)

| Element | Wymaganie |
|---|---|
| Zestaw | ≥ 100 przypadków injection: tekst (strona, plik, mail), obraz/ekran (OCR), dźwięk (nagranie z komendą, TV), opis narzędzia MCP; tworzy **model-recenzent**, zamrożony w `evals/F3/redteam/` |
| Próg | **0 eskalacji uprawnień, 0 egressu bez potwierdzenia** (na L3 i L4) |
| Dodatkowo | ≥ 100 scenariuszy „agentka zmienia Jądro / zatwierdza sama siebie" = 0 sukcesów (F3-04) |
| Powtórzenie | F9 na pełnym systemie (F9-07) + pentest modelem innym niż autor wg tego dokumentu i OWASP: 0 otwartych ustaleń CVSS ≥ 9 (F9-01) |
| Utrzymanie | każdy nowy wektor (nowy `tools-*`, nowy most, nowa klasa treści) dodaje przypadki do zestawu przed merge modułu |

## 11. Otwarte kwestie (do domknięcia w F3)

- Kotwica głowy łańcucha hashy audytu: gdzie fizycznie (plik ACL na koncie Brokera vs TPM) — ADR w F3.
- Zakres helpera `uiAccess` i jego weryfikacja wywołującego (Authenticode + SID).
- Czy Windows Sandbox/VM ma być obowiązkowa dla shella spoza zakresu na L4 — decyzja właściciela.
- Poziom izolacji `tools-browser` (własny profil + rozszerzenie) wobec sesji zalogowanych za zgodą.
