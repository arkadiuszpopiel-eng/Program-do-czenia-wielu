# Alfa — plan programu

> Lekki, bardzo modułowy program dla Windows, który łączy modele lokalne, chmurowe, API i zwykłe plany abonamentowe w jedną rozmowę — tekstową i głosową — z pełnym dostępem do komputera. Cztery agentki na start (**Alfa, Beta, Gama, Delta**) z własnymi głosami, pamięcią, samonaprawą i samodoskonaleniem; role przydzielane dowolnie. Całość budowana w 100% przez modele AI (Claude Opus 5.5 i GPT-6 Sol).
>
> Wersja 5 — po Twoich sprecyzowaniach i odpowiedziach (użytek osobisty, budowa przez AI, nazwa Alfa, cztery agentki, rozbudowany głos, lekkość i modularność, **sprzęt minimalny (baseline) Ryzen 5 5600 + RX 7600 8 GB + 16 GB RAM + Win11 — tu program ma działać normalnie; Twoje maszyny: desktop RX 9070 XT i laptop RTX 4050**, API do zaawansowanej rozmowy, otwarte mosty, wysoka autonomia, klucze dodawane później, import/eksport zamiast synchronizacji), po trzech niezależnych przeglądach (architektura, szczegóły i UI/UX, roadmapa) i badaniach (29–30.09.2026).

---

## 0. Kontekst i cel

**Stan repo:** `Program-do-czenia-wielu` jest puste (brak commitów) → projekt od zera. Ten dokument to **plan**, nie kod.

| Wymaganie | Gdzie |
|---|---|
| Modele lokalne + chmurowe + API + zwykłe plany abonamentowe (bez API) | §5 (+ zgodność §1.3) |
| Wszystkie funkcje aplikacji Windows / „używanie komputera" | §7 |
| **Bardzo rozbudowany system głosowy**, rozmowa jak z człowiekiem, przerywanie i korekta | §6 |
| Agentki **Alfa, Beta, Gama, Delta** — żeńskie, z żeńskimi głosami (brzmienie młodej dorosłej, ok. 18–25 lat), role dowolnie przydzielane | §9 + §6.6 |
| Agenci dla podsystemów, agent ustalający reguły i kolejność | §9 |
| 100% dostęp do systemu (zapis, kopiowanie, katalogi…) | §7 + §8 |
| Samodoskonalenie, samonaprawa problemowa | §12 |
| Pamięć sesji, wiele czatów z osobną pamięcią, oddawanie plików | §10, §11 |
| Bardzo rozbudowane logi i postępy | §13 |
| UI minimalistyczne na wierzchu, pełne ustawienia pod spodem, chowane panele | §14, §15 |
| **Lekki i bardzo modularny** | §3 |
| **Budowany w 100% przez AI (Opus 5.5 / GPT-6 Sol)** | §4 |

---

## 1. Decyzje i zastrzeżenia

### 1.1 Twoje sprecyzowania → skutki w planie

| # | Decyzja | Skutek |
|---|---|---|
| 1 | Użytek osobisty, tylko dla Ciebie | Brak dystrybucji: bez usługi podpisywania kodu (wystarczy własny certyfikat lokalny), bez audytu licencji „pod dystrybucję" (modele niekomercyjne OK), bez telemetrii, uproszczone RODO/AI Act. Zgodność tras abonamentowych łatwiejsza (§1.3) |
| 2 | Budowa w 100% przez Opus 5.5 lub GPT-6 Sol | Cały §4: małe moduły, kontrakty, symulatory, bramki CI, przegląd krzyżowy, lista bramek ludzkich |
| 3 | Nazwa programu: **Alfa** | W dokumentach: „Alfa (program)" vs „Alfa (agentka)"; w kodzie `alfa-*` (program) i `agent.alfa` (agentka) |
| 4 | **Cztery agentki na start: Alfa, Beta, Gama, Delta** (wszystkie żeńskie, z żeńskimi głosami młodych dorosłych 18–25 lat — potwierdzone). Program i pierwsza agentka noszą to samo imię „Alfa" | §9.2, §6.6 |
| 5 | System głosowy bardzo rozbudowany | §6: Voice Suite z ~16 modułów |
| 6 | Technologia lekka i bardzo modularna | §3: mikrojądro + moduły, budżety lekkości |
| 7 | **Sprzęt minimalny (baseline) — na nim program ma działać normalnie** (nie Twój obecny): Ryzen 5 5600 (6c/12t, bez iGPU), **Radeon RX 7600 8 GB (AMD — bez CUDA)**, 16 GB RAM, Windows 11. Twoje maszyny: **desktop** Ryzen 7 5700X3D · RX 9070 XT 16 GB · 32 GB oraz **laptop** i7-13700H · RTX 4050 6 GB · 16 GB — na nich więcej lokalnie i na nich testy | §3.4 (budżety liczone na baseline), §3.5 (wiele maszyn), §6.3 (profile A–D), §1.2 |
| 8 | **Role agentek zmienne** — imię/głos/charakter to persona, rola jest przydzielana zależnie od potrzeb | §9.2 „Obsada ról" |
| 9 | **API do zaawansowanej rozmowy** (chmurowe modele + opcjonalnie chmurowy głos), lokalnie to, co lekkie i szybkie | §5.2, §6.3 |
| 10 | Mosty otwarte: na start Claude Code i Codex; Gemini, Grok, modele chińskie do dodania | §5.5 |
| 11 | Autonomia **bardzo wysoka lub maksymalna**, zmienna w ustawieniach | §8.3 |
| 12 | Jakość: „ma być idealnie i dobrze" → sztywna Definition of Done | §4.4 |

*Zatwierdzone przez Ciebie: kolejność „głos przed computer use"; wiek głosów 18–25 (brzmienie młodej dorosłej, nie dziecięce); AI pracuje lokalnie na Twoim Windows (głos/UIA) + w chmurze (reszta); automerge po zielonym CI + przeglądzie drugiego modelu, Jądro/Broker zawsze przez Ciebie.*

### 1.2 Decyzje techniczne (każda odwracalna do końca F0 przez ADR)

| Obszar | Decyzja | Uzasadnienie / alternatywa |
|---|---|---|
| Platforma | **Windows 11 x64** (Win10 best effort; ARM64 poza v1). OS-specyficzne rzeczy za traitem `SystemPort` | „Funkcje aplikacji Windows" |
| Rdzeń | **Rust** workspace | Bezpieczeństwo pamięci, audio RT, kompilator jako pętla zwrotna dla AI |
| Powłoka UI | **Tauri 2.x + WebView2** (zostać na 2.x; Tauri 3 to alpha) | Najlepszy kompromis lekkość / bogaty UI / dostępność. Uwaga: RAM to głównie WebView2 (Chromium) — **nie obiecujemy „5× mniej niż Electron"**; F0 mierzy sumę Private Working Set całego drzewa procesów przy 1 i 3 oknach. Zamknięte okno = ukryte przez N minut, potem zniszczony WebView (§14.2) |
| Frontend | **Svelte 5 (runes) + Bits UI**, czysty Vite, bez SSR. Plan B: React 19 + React Aria | Lekkość + dostępność. Ryzyko: modele mieszają składnię Svelte 4/5 → egzekwujemy `svelte-check` + `eslint-plugin-svelte` w CI/hookach, w AGENTS.md zakaz `export let`, `$:`, `on:`; oficjalny Svelte MCP. Solid odradzany (2.0 w RC) |
| Audio | **crate `wasapi`** (loopback, tryb zdarzeniowy, per-proces); `cpal` nie do loopbacku | cpal ma ograniczoną kontrolę bufora i problemy z loopbackiem WASAPI |
| ML | **Domyślnie bez Pythona:** whisper.cpp (**backend Vulkan** pod AMD, wersja przypięta — min. 1.8.1, fallback CPU), ONNX Runtime na **CPU**, sherpa-onnx. Python tylko opcjonalny „Model Pack" | Lekkość. **Baseline ma kartę AMD → bez CUDA; na maszynach z NVIDIA moduły CUDA są opcjonalnie dostępne.** DirectML jest w trybie maintenance; małe modele (VAD, turn, KWS, embedding mówcy) na CPU. PyTorch-ROCm na Windows dla gfx110X jest niestabilny → **na baseline Chatterbox / XTTS-v2 / F5 odpadają lokalnie** (RTF ≥ 1; na mocniejszych maszynach z NVIDIA/CUDA są dostępne jako opcjonalne moduły); na baseline jakość TTS ponad lokalny Pocket/Piper = chmura |
| Lokalny LLM | **llama.cpp z Vulkan/CUDA** (nie Ollama — opóźniony llama.cpp), modele 3–4,5B Q4_K_M (np. Bielik 4.5B; pobierany w onboardingu, gdy nie ma kluczy), bez kwantów IQ (crashe na RDNA3/Vulkan); 8B tylko gdy STT idzie na CPU | 8 GB VRAM: pulpit 0,5–1 GB + STT 1–2,5 GB → zostaje ~4,5–6 GB. Ryzyko: tool-use po polsku na 3–4,5B — próg jakości mierzony w F3 |
| Dane | SQLite (WAL) z szyfrowaniem (SQLCipher — zgodność z sqlite-vec sprawdzana w F0) + **sqlite-vec** (wektory) + **FTS5** (wyszukiwanie pełnotekstowe) w tej samej, szyfrowanej bazie per sesja → usunięcie klucza sesji kasuje też indeks | Lokalnie, lekko; jeden silnik zamiast dwóch |
| Wtyczki AI | **wasmtime + własny WIT**, cel `wasip2` + wit-bindgen (WASI 0.3 zbyt świeże); brak importów WASI, limity epoch/fuel/pamięci | Sandbox dla kodu generowanego przez AI |
| Kontrakty | Rust traity → typy TS generowane (`tauri-specta`/`ts-rs`, wersje przypięte), JSON Schema dla zdarzeń, WIT dla Wasm; w CI `git diff --exit-code` na plikach generowanych | Brak dryfu między warstwami |
| Instalacja | Wersje side-by-side w `%LOCALAPPDATA%\Alfa\versions\<ver>` + **stały launcher `%LOCALAPPDATA%\Alfa\alfa.exe`** (stała ścieżka dla skrótu w Menu Start/AUMID toastów, autostartu, handlera protokołu, „Wyślij do", skrótów globalnych) + **stały folder danych WebView2** poza katalogiem wersji; helper `uiAccess` instalowany osobno w `Program Files` (jednorazowy UAC); aktualizacje z własnego repo z podpisem minisign (własny klucz); rollback = przełączenie launchera na poprzednią wersję | Użytek osobisty; bez MSIX (wirtualizacja rejestru) |
| Język | Polski + angielski, i18n od dnia 0 | |
| Telemetria | **Brak** | |

### 1.3 Plany abonamentowe „zamiast API" — co jest zgodne z regulaminami (sprawdzone 29.09.2026)

- **Anthropic:** third-party nie mogą oferować logowania Claude.ai, kierować ruchu przez dane logowania planów ani przechowywać tokenów; **dozwolone** jest logowanie użytkownika własnym planem do **niezmodyfikowanego binarnego Claude Code**. Limity Pro/Max zakładają zwykłe, indywidualne użycie; zasady Agent SDK są w ruchu (kredyt SDK ogłoszony i wstrzymany 15.06.2026).
- **Google:** bezpośredni dostęp do usług Gemini CLI przez oprogramowanie trzecie = naruszenie; od 18.06.2026 osobiste logowanie do Gemini CLI dla planów konsumenckich wygaszono.
- **OpenAI:** Codex CLI oficjalnie działa z planami ChatGPT; „bring your own plan" dla aplikacji trzecich bez oficjalnej odpowiedzi → ostrożnie.

**Zasady (użytek osobisty upraszcza, ale nie znosi ich):**
1. **Most abonamentowy = sterowanie oficjalnym, niezmodyfikowanym CLI** (`claude`, `codex`), do którego **logujesz się Ty**, przez jego oficjalne tryby nieinteraktywne/SDK/app-server. **Kod Alfy nigdy nie czyta ani nie przechowuje tokenów** tych narzędzi.
2. Most = **`AgentBackend`** (zadanie → strumień zdarzeń), „opaque worker" (§8.5), nie surowe `chat.completions`.
3. **Egzekucja techniczna:** deny-lista domen/aplikacji dostawców w GUI/przeglądarce (agent nie „używa" ich webowego UI); deny-lista ścieżek poświadczeń (`~/.claude`, `~/.codex`, profile przeglądarek, Credential Manager) w `fs.*`/shellu; przypięte wersje CLI (nieznana wersja wyłącza trasę); osobne statusy „CLI `-p`" i „Agent SDK".
4. **Uruchamianie:** domyślnie na Twoje żądanie; harmonogramy mostów tylko jawnie włączone przez Ciebie, z limitami; Ulepszacz (§12) mostów nie używa.
5. **Rejestr zgodności** (`compliance-registry.json`): status, data weryfikacji, cytat źródła + archiwalna kopia regulaminu, wyłącznik trasy, karta zgodności w UI; nieświeży rejestr degraduje trasę do „szarej".
6. **Wykluczone:** automatyzacja webowych UI dostawców, wyciąganie ciasteczek/tokenów.
7. Redukcja kosztów: modele lokalne (głos, zadania tanie), mosty CLI do ciężkiej pracy, API tam, gdzie się opłaca.

### 1.4 „100% dostępu" a bezpieczeństwo
Pełna moc (poziom „Max"/L4) jako Twój świadomy wybór, z zabezpieczeniami, których agent nie zmieni (§8). Uczciwie: przy L4 audyt jest „best effort" (§8.7).

---

## 2. Zasady projektowe

1. **Lekkość** — moduł nieużywany nie kosztuje pamięci ani CPU (ładowanie na żądanie, zwalnianie po bezczynności).
2. **Mikrojądro** — jądro jest maleńkie; wszystko inne to wymienialny moduł z kontraktem.
3. **Zbudowane dla AI** — małe moduły mieszczące się w kontekście, jeden kontrakt na moduł, fake'i do testów bez sprzętu.
4. **Wszystko jest zdarzeniem** (event sourcing): logi, postępy, cofanie, replay, samodoskonalenie.
5. **Przerywalność wszędzie** — głos, tekst, generowanie, narzędzia, zadania w tle.
6. **Najmniejsze uprawnienia**, potomek ≤ rodzic; reguły mogą tylko zawężać.
7. **Jądro bezpieczeństwa poza zasięgiem agentów** — osobny proces/konto.
8. **Prywatność przez tagi** sesji, egzekwowana także dla mostów CLI i chmurowego głosu.
9. **Fallback na każdej ścieżce** (API → UIA → wizja; model A → B; chmura → lokalnie; offline).
10. **Mierzalność** — każdy cel ma metrykę, zestaw testowy i próg go/no-go.
11. **Spokój na wierzchu, moc pod spodem** (UI).

---

## 3. Architektura: mikrojądro + moduły

### 3.1 Schemat
```
┌────────────────── UI (Tauri 2 · WebView2 · Svelte 5) ──────────────────┐
│ Powierzchnia │ Panele chowane: Sesje·Agentki·Oś czasu·Pliki·Pamięć·Ekran·Głos │ Ustawienia│
└───────────────▲────────────────────────────────────────────────────────┘
                │ typowane komendy + strumień zdarzeń (minimalne capabilities per okno)
┌───────────────┴────────────── JĄDRO „Alfa Core" (Rust, ≈ kilka MB) ────────────────┐
│ Magistrala zdarzeń · Rejestr modułów · Konfiguracja · Klient Brokera · Log-writer   │
│ (bez logiki domenowej; wszystko poniżej to moduły z manifestem)                     │
└──▲────────────────────────▲────────────────────────▲──────────────────────▲───────┘
   │ in-proc (feature flag) │ osobny proces (stdio/pipe)│ Wasm (wasmtime+WIT)   │
┌──┴───────────┐ ┌──────────┴────────┐ ┌────────────────┴───┐ ┌───────────────┴────┐
│ Krytyczne:   │ │ Ciężkie/awaryjne: │ │ Wtyczki AI,        │ │ Zewnętrzne:        │
│ voice-audio, │ │ STT/TTS/LLM local,│ │ umiejętności       │ │ serwery MCP, CLI   │
│ scheduler,   │ │ przeglądarka,     │ │ (sandbox, limity)  │ │ (mosty, „opaque    │
│ memory, router│ │ UIA-helper        │ │                    │ │ worker")           │
└──────────────┘ └───────────────────┘ └────────────────────┘ └────────────────────┘
        ┌──────────────────────────────────────────────────────────────┐
        │ BROKER (usługa w tle, osobne konto): tokeny zdolności,       │  + BROKER-UI (okno
        │ writer audytu, kill-switch, polityki Jądra, operacje z UAC   │  zatwierdzeń w Twojej
        └──────────────────────────────────────────────────────────────┘  sesji, wyższy poziom
        + WATCHDOG (osobny proces: restart, safe-mode, rollback, Job Objects)  integralności)
```

### 3.2 Moduł = jednostka wszystkiego
- **Trójka crate'ów:** `<m>-contract` (trait + typy + zdarzenia + JSON Schema), `<m>-impl`, `<m>-fake` (atrapa do testów). Inne moduły zależą **tylko od `-contract`**.
- **Manifest `module.toml`:** `id`, `version`, `kind` (service/tool/provider/voice-engine/agent-pack/ui-panel), kontrakty dostarczane/wymagane, żądane zdolności, **budżet zasobów** (RAM/CPU), cykl życia (`lazy|on-demand|always`), izolacja (`inproc|process|wasm`), schemat konfiguracji, wkład do UI (panel, strona ustawień), health-check.
- **Izolacja wg potrzeby:** audio i scheduler — natywny crate, wątek RT, **bez Wasm/IPC w callbacku audio**; ciężkie/awaryjne (STT/TTS/LLM lokalne, przeglądarka) — osobny proces (JSON-RPC po stdio/named pipe; AppContainer + Job Object dla niezaufanych); wtyczki generowane przez AI — Wasm. DLL (`abi_stable`) odrzucone.
- **Kompozycja:** feature flags wybierają skład builda (minimalny = sam czat); w runtime włączanie/wyłączanie modułów (hot dla proces/Wasm, aktywacja flagą dla in-proc).
- **Wątki:** UIA na dedykowanym wątku MTA; windows-rs/COM zamknięte w jednym crate `platform-windows` za `SystemPort`.

### 3.3 Katalog modułów (skrót)
| Grupa | Moduły |
|---|---|
| **Jądro** | `core-bus`, `core-registry`, `core-config`, `core-log`, `updater` (launcher, wersje, rollback) |
| **Bezpieczeństwo** | `safety-broker` (usługa) + `broker-ui` (okno zatwierdzeń), `watchdog`, `risk-classifier`, `undo-journal`, `compliance` (rejestr tras) |
| **Modele** | `accounts-hub` (konta i klucze, §5.6), `providers-local`, `providers-api` (Anthropic, OpenAI, Google…), `agent-backends` (mosty CLI), `router`, `cost-meter` (koszty, limity, kurs PLN), `model-residency` (zarządca RAM/VRAM) |
| **Agentki** | `agent-runtime`, `personas`, `scheduler` (+lock manager; `scheduler-lite` już w F2), `marshal` (tłumacz reguł), `agent-builder`, `triggers` (harmonogram i wyzwalacze) |
| **Głos** | `voice-*` (§6.2) |
| **System** | `platform-windows`, `tools-fs`, `tools-shell`, `tools-clipboard` (z historią), `tools-window`, `tools-uia`, `tools-vision`, `tools-input`, `tools-browser`, `tools-office`, `tools-system` (procesy, usługi, rejestr, ustawienia), `tools-net`, `tools-media`, `mcp` (klient + serwer), `shell-integration` (zasobnik, toasty, protokół, „Wyślij do", pasek zadań), `notify` |
| **Dane** | `memory`, `sessions`, `search` (FTS + wektory), `artifacts`, `transfer` (import/eksport, kopie zapasowe) |
| **Sprzęt** | `device-profile` (autodetekcja, profil per maszyna, tryb baterii) |
| **Samo-ulepszanie** | `diagnostician`, `improver`, `evals`, `plugin-runtime` |
| **UI** | `ui-shell`, `ui-kit` (tokeny, komponenty), `ui-quick` (Szybkie pytanie, pigułka głosowa, menu zasobnika), `ui-terminal` (ConPTY do logowania CLI), panele jako moduły UI ładowane leniwie |

### 3.4 Budżety lekkości (wstępne — wartości ostateczne po pomiarze w F0, egzekwowane testami)
| Wskaźnik | Cel wstępny |
|---|---|
| Instalacja bazowa (jądro + UI, bez modeli) | ≤ 40 MB; modele/moduły pobierane na żądanie |
| Start procesu do zasobnika / do okna (zimny start aplikacji) | ≤ 1 s / ≤ 1,5 s (osobna metryka: ponowne otwarcie okna z zasobnika — §14.2) |
| Idle bez głosu: jądro | ≤ 40 MB Private Working Set, CPU ≈ 0 |
| Idle z oknem | suma drzewa procesów **zmierzona** (WebView2 dominuje), cel wyznaczony w F0 |
| Moduły głosowe | ładowane przy pierwszym użyciu, zwalniane po bezczynności (konfigurowalnie) |
| **Sprzęt minimalny — baseline (na nim MA działać normalnie)** | Ryzen 5 5600 · RX 7600 8 GB (Vulkan, bez CUDA) · 16 GB RAM · Windows 11 — wszystkie budżety i kryteria DoD mierzone na baseline; mocniejsze maszyny mogą je tylko poprawiać |
| RAM z załadowanym głosem | szacunek ~2,5–4 GB łącznie (WebView2 300–500 MB + whisper Q5 1–1,5 GB + Pocket TTS 0,5–1,5 GB + reszta), zapas ≥ 7 GB z 16; **do zmierzenia w F0** |
| VRAM (8 GB) | pulpit 0,5–1 GB + STT 1–2,5 GB; lokalny LLM 3–4B Q4 obok STT; 8B tylko przy STT na CPU |
| Zarządca rezydencji modeli | limit VRAM/RAM; STT+TTS+LLM nie zawsze naraz; bez wyścigu o VRAM z grami (wykrywanie pełnego ekranu → STT/LLM na CPU lub chmurę) |
| Budżet per moduł | deklarowany w manifeście, monitorowany, przekroczenie → ostrzeżenie/zwolnienie |
| Binaria | LTO, strip; opt-level pod RT tylko tam, gdzie trzeba |

### 3.5 Wiele maszyn i klasy sprzętu
Program jest projektowany **od dołu (baseline) w górę**; Twoje dwie maszyny to dwie różne ścieżki GPU, więc obie są w macierzy testów:
| Klasa | Sprzęt | Co daje / ograniczenia |
|---|---|---|
| **Baseline (minimum — tu ma działać normalnie)** | Ryzen 5 5600 · RX 7600 8 GB (AMD RDNA3) · 16 GB · Win11 | profil głosu A/B/C, lokalny LLM tylko mały (3–4B), reszta przez API; wszystkie DoD i budżety. **Nie masz go fizycznie → emulacja limitów + proxy z Twoich maszyn** |
| **Twój desktop** *(klasa Standard-AMD)* | Ryzen 7 5700X3D (8c/16t, V-Cache) · **RX 9070 XT 16 GB (AMD RDNA4)** · 32 GB RAM | ścieżka **Vulkan** (whisper.cpp large-v3 pełny, llama.cpp 8–14B Q4 lokalnie z zapasem VRAM). Brak CUDA; HIP/ROCm dla RDNA4 na Windows do sprawdzenia w F0 (zgłoszono `ErrorDeviceLost` na RDNA4 w części narzędzi → przypięte wersje + fallback CPU). Lokalny TTS wysokiej jakości (Chatterbox/XTTS/F5) tylko jeśli spike PyTorch-ROCm/port Vulkan na RDNA4 wypadnie dodatnio; inaczej Pocket-PL (CPU) / chmura |
| **Twój laptop** *(klasa Laptop-CUDA)* | Intel i7-13700H (14c/20t) · **RTX 4050 6 GB (NVIDIA)** · 16 GB RAM | ścieżka **CUDA**: whisper.cpp CUDA (turbo Q5 mieści się), LLM lokalny 3–4B; **tylko 6 GB VRAM** (mniej niż baseline) → STT i ciężki TTS nie są rezydentne naraz (zarządca rezydencji, wymiana modeli); mocny CPU (Pocket-PL na CPU jest komfortowy); **tryb baterii** i limit termiczny |
| **Przyszły „Mocny"** | NVIDIA ≥ 12–16 GB lub inne mocne GPU | pełny lokalny stos (profil D) |

- **Moduł `device-profile`:** autodetekcja przy pierwszym uruchomieniu i po zmianie sprzętu (CPU, RAM, producent i model GPU, VRAM, NPU, bateria, urządzenia audio) → dobór profilu potoku i modułów (§6.3) **per maszyna**; „Kreator sprzętu" pokazuje kompromis wprost i pozwala nadpisać.
- **Konfiguracja warstwowa:** wspólna (agentki, obsada, biblie głosów, reguły, uprawnienia) + **nakładka per maszyna** (`config/machine/<id>.toml`: urządzenia audio, profil głosu, moduły rezydentne, limity zasobów).
- **Przenoszenie między maszynami = import/eksport do pliku, bez automatycznej synchronizacji** (moduł `transfer`, §15.1). Klucze API i sekrety nie wchodzą do eksportu (Credential Manager jest per maszyna; **decyzja 2026-10-04 (CX-a): AGENTS.md wygrywa — brak osobnego eksportu sekretów**).
- **Uwaga do emulacji:** Twój 5700X3D ma 96 MB L3 (5600 — 32 MB), więc czasy CPU (whisper/LLM/TTS) wyjdą **zawyżone na korzyść** → do wyników CPU doliczamy margines +20–30% albo mierzymy dodatkowo na laptopie ograniczonym do 6 rdzeni P. **GPU:** RX 9070 XT ma ~640 GB/s przepustowości pamięci, RX 7600 ~288 GB/s → czasy whisper.cpp/llama.cpp na Vulkanie z desktopu mnożymy przez **~2,2** (albo ścieżka CPU jako górna granica) przy ocenie kryteriów baseline.
- **Testowanie baseline:** baseline'u nie masz fizycznie → **emulacja ograniczeń** (limity RAM i CPU przez Job Objects/affinity do 6 rdzeni/16 GB, budżet VRAM 8 GB w zarządcy rezydencji) na Twoich maszynach; **laptop** jest dobrym testem ciasnego VRAM (6 GB) i 16 GB RAM (ścieżka CUDA), **desktop** — ścieżki Vulkan/AMD (RDNA4). **Luka:** RDNA3/RX 7600 nie jest pokryty fizycznie — stabilność Vulkan/whisper.cpp na tej generacji pozostaje ryzykiem (fallback CPU i przypięte sterowniki).

### 3.6 Układ repo
```
apps/desktop/         Tauri 2 shell + ui/ (Svelte 5)
crates/<moduł>-contract|-impl|-fake   (wg katalogu §3.3)
crates/core-*         jądro
sidecars/             ciężkie moduły jako osobne procesy (whisper.cpp, TTS…)
plugins/              źródła wtyczek Wasm + WIT
packages/ui-kit/      tokeny designu, komponenty, Storybook
packages/schemas/     JSON Schema zdarzeń i konfiguracji
providers-catalog/    deklaratywny katalog dostawców (§5.6)
evals/                zadania wzorcowe, zamrożone zestawy akceptacyjne fal (hash), korpus własny (poza gitem), holdout, benchmarki głosu
docs/                 PLAN.md, ARCHITECTURE.md, AI_WORKFLOW.md, ACCEPTANCE.md, VOICE.md, PERSONAS.md, UI.md,
                      THREAT_MODEL.md, ADR/, modules/<m>/SPEC.md, formats/, vendor/ (skróty zależności),
                      compliance/ (subscription-routes.md, compliance-registry.json, archive/)
AGENTS.md  CLAUDE.md  README.md
```

---

## 4. Budowa w 100% przez AI (Opus 5.5 / GPT-6 Sol)

### 4.1 Wykonawcy i podział pracy
- **Claude Code + `claude-opus-5-5`** i **Codex CLI + GPT-6 Sol**, oba przez Twoje własne logowanie (zgodne z §1.3).
- **Domyślny podział:** Opus 5.5 (Claude Code) — Rust: jądro, Broker, głos, narzędzia systemowe; GPT-6 Sol (Codex) — UI/Svelte, testy E2E, dokumentacja. Podział można odwrócić per moduł; liczy się zasada niżej.
- **Przegląd krzyżowy:** kto napisał moduł, tego nie recenzuje; recenzję robi drugi model w świeżym kontekście, ograniczoną do poprawności (nie „znajdź cokolwiek"). Inna rodzina modeli łapie skorelowane ślepe punkty. **SPEC pisze autor, akceptuje recenzent** (Jądro/Broker — Ty).
- **Zestawy akceptacyjne fal tworzy model-recenzent** (nie autor), Ty je akceptujesz, a potem są **zamrożone** (hash w `evals/`) przed implementacją — inaczej autor pisze testy pod własny kod.
- **Backlog = GitHub Issues** w kolejności §16.2; jedna karta zadania = jedna sesja.
- **Jedna sesja = jeden moduł = jeden git worktree**; granice modułów zapobiegają konfliktom. Jądro i Broker: przegląd zawsze także przez Ciebie.
- Kontekst nie jest gwarantowany (w Codex CLI zgłaszano przycięcie kontekstu GPT-6 Sol) → **moduł ma mieścić się w małym oknie** niezależnie od modelu.

### 4.2 Środowisko
- **Windows lokalnie (Twoje maszyny):** tu AI uruchamia testy audio/UIA/GPU i benchmarki — wymagane dla modułów zależnych od sprzętu. **Desktop (RX 9070 XT, Vulkan/AMD)** jako główna maszyna dev + runner dla ścieżki AMD; **laptop (RTX 4050, CUDA)** jako runner dla ścieżki NVIDIA i testów baterii/ciasnego VRAM.
- **Chmura (jak ta sesja) + GitHub Actions `windows-latest`:** moduły niezależne od sprzętu (jądro, scheduler, pamięć, router, UI z atrapami), **tylko build, lint i testy jednostkowe** (minuty Windows w prywatnym repo liczą się podwójnie; pomiary wydajności na hostowanym runnerze są zaszumione); opcjonalnie kompilacja krzyżowa z Linuksa.
- **Self-hosted runner (desktop, emulacja baseline) jest wymaganym checkiem przed merge** dla modułów sprzętowych i wydajnościowych (`voice-*`, `providers-local`, `platform-windows`, budżety UI §14.7) — inaczej DoD pkt 4 nie da się spełnić. **Nightly:** pełna macierz desktop + laptop.
- **Bezpieczeństwo runnera:** kod pisany przez AI i automatycznie mergowany wykonuje się na Twoim desktopie → runner działa na **osobnym koncie Windows bez dostępu do Twojego profilu**, repo prywatne, bez PR z forków.

### 4.3 Pliki sterujące i procedura
- `AGENTS.md` (≤ ~100 linii; pisze go AI bez `/init`, Ty zatwierdzasz; tylko rzeczy niewyprowadzalne z kodu) + cienki `CLAUDE.md` z `@AGENTS.md` (na Windows bez symlinku); egzekucja regułami w **hookach**, nie w prozie. Codex przycina AGENTS.md powyżej 32 KiB.
- **`docs/modules/<m>/SPEC.md` (1 strona):** cel, kontrakt, niezmienniki, testy akceptacyjne, budżety. **Karty zadań** dla agentów.
- **`docs/vendor/<crate>.md` (skróty zależności ≤ 2 strony):** dokładnie używane API, zweryfikowane kompilacją. Powód: **wiedza modeli kończy się w IV–VI 2026** — WASI 0.3, wasmtime 46, Tauri 3 alpha, windows-rs 0.100, tauri-specta RC są dla nich świeże. Zasada: przypinaj wersje, które modele znają, albo dawaj im dokumentację (docs.rs). windows-rs: jeden crate, jedna wersja.
- **Kolejność pracy nad modułem:** SPEC → kontrakt → fake → testy (najpierw) → implementacja → przegląd drugiego modelu → CI → merge.
- **Heurystyki rozmiaru (egzekwowane narzędziami):** plik ≤ ~300–400 linii, crate ≤ ~5–8 tys. linii; clippy `too_many_lines`, `cargo-deny`, sprawdzanie grafu zależności (moduł → tylko `-contract`), `dependency-cruiser` dla TS. *(Liczby to heurystyka, nie wynik badań.)*

### 4.4 Bramki jakości (CI) i Definition of Done („ma być idealnie i dobrze")
**CI:** `cargo fmt`, `clippy -D warnings`, `cargo test`, `cargo-deny/vet` + przegląd `Cargo.lock` przy nowym crate (halucynowane crate'y/API), `svelte-check` + eslint, `git diff --exit-code` na kontraktach generowanych, axe (a11y), E2E (Playwright przez CDP do WebView2 albo `tauri-driver`), eval-e, budżety lekkości i opóźnień na baseline (emulowanym lub fizycznym).

**Definition of Done modułu (bez spełnienia wszystkiego — brak merge):**
1. `SPEC.md` aktualny; kontrakt + `-fake` + testy kontraktowe zielone.
2. Testy jednostkowe i właściwościowe (property-based) dla logiki; progi pokrycia dla modułów nie-UI (wstępnie ≥ 85% linii, ≥ 70% gałęzi, do ustalenia w F0); testy regresyjne dla każdego naprawionego błędu.
3. Zero `TODO/FIXME/unimplemented!/unwrap()` w ścieżkach produkcyjnych (lint), zero ostrzeżeń.
4. Budżety zasobów modułu (RAM/CPU/opóźnienie) zmierzone na **baseline emulowanym** (§3.4–3.5) i spełnione — dla modułów z F0–F1 progi wstępne, zaostrzane po pomiarach F0.
5. Obsługa błędów i stany brzegowe (§14.4) pokryte testami; logi/zdarzenia zgodne ze schematem (§13).
6. Przegląd drugiego modelu w świeżym kontekście (poprawność, bezpieczeństwo, zgodność ze SPEC) — wszystkie uwagi zamknięte.
7. Lista kontrolna bezpieczeństwa (uprawnienia, wejście niezaufane, sekrety) i dokumentacja użytkownika/ustawień.
8. **Merge tylko na zielono**; moduły Jądra/Brokera i zmiany polityk bezpieczeństwa dodatkowo przez Ciebie.

*Uczciwie: „idealnie" nie da się zagwarantować — dlatego jest to lista mierzalnych kryteriów, a nie deklaracja; wyniki tych bramek widzisz w każdym PR.*

### 4.5 Symulatory i fake'i (żeby AI mogło weryfikować bez ludzi)
Fake audio (odtwarzanie WAV + wirtualny zegar), fake LLM (record/replay), fake UIA (drzewa z fixture'ów), fake `SystemPort` (wirtualny system plików), fake dostawca TTS/STT. Potok głosowy testowany deterministycznie z wirtualnym zegarem.

### 4.5a Pierwsze dwa tygodnie (co powstaje najpierw w F0, w tej kolejności)
1. *(Opus 5.5; recenzja GPT-6 Sol)* Repo: workspace Rust + `apps/desktop` (Tauri 2 + Svelte 5 „hello"), `AGENTS.md`, `CLAUDE.md`, hooki (fmt/clippy/svelte-check), CI `windows-latest` zielone na pustym projekcie; `docs/THREAT_MODEL.md` szkic.
2. *(Opus 5.5; recenzja GPT-6 Sol)* Kontrakty jądra `core-bus/registry/config/log` + manifest modułu (schemat) + **`SystemPort`-contract z fake'iem** + pierwszy moduł-przykład z trójką crate'ów i testem kontraktowym — **wzorzec dla wszystkich kolejnych**; szkielet `evals/` i harnessu `voice-lab`.
3. *(GPT-6 Sol; recenzja Opus 5.5)* `packages/ui-kit` v0: tokeny (§14.3) + podstawowe komponenty + Storybook + makiety 1–3 (start, rozmowa, tryb głosowy) → **Twoja akceptacja** (bramka #4).
4. Spike (f) RAM/start Tauri (1 i 3 okna) → wstępne budżety §3.4 i §14.7.
5. Spike (a) pętla głosowa mic → VAD → whisper.cpp → Pocket-PL → głośnik z barge-in na desktopie i laptopie; równolegle **nagranie korpusu** (bramka #3).
6. Spike (h) pomiary sprzętowe + spike (e) tabela kandydatów głosu → ADR (4) i ADR (11); spike (i) dane → ADR (8); spike (k) Broker-UI → ADR (3).
Każdy punkt to osobna sesja AI z własnym worktree.

### 4.6 Bramki ludzkie (tego AI nie zrobi za Ciebie)
1. Logowanie do CLI (Claude Code, Codex) i klucze API.
2. Potwierdzenia UAC i (opcjonalnie) Windows Hello; podniesienie poziomu do L4 „Maks" (L3 jest domyślny).
3. **Nagranie korpusu własnego** (~30–45 min Twojej mowy: różne warunki) — do oceny STT, treningu słów wywoławczych i weryfikacji mówcy.
4. **Akceptacja makiet UI** (§14.10) przed implementacją widoków.
5. **Odsłuchy:** casting czterech głosów (Alfa/Beta/Gama/Delta), potwierdzenie „brzmi jak młoda dorosła", wybór końcowy.
6. Testy akustyczne na Twoich maszynach (desktop i laptop; mikrofon/głośniki/słuchawki, w tym wbudowany mikrofon i głośniki laptopa — najtrudniejszy przypadek dla AEC).
7. Decyzje z §19 i akceptacja PR-ów Jądra/Brokera; **akceptacja ADR-ów do końca F0** i SPEC-ów Jądra/Brokera; uruchamianie sesji deweloperskich.
8. **Przygotowanie maszyn:** toolchainy na desktopie i laptopie (Rust MSVC, VS Build Tools, Node, Vulkan SDK, CUDA), przypięte sterowniki Adrenalin/NVIDIA, wirtualny kabel audio, VM (Hyper-V wymaga Win11 Pro — edycja do potwierdzenia).
9. **Repozytorium i runnery:** repo prywatne na GitHub, rejestracja self-hosted runnerów na **osobnym koncie Windows**, uprawnienia.
10. **Sekrety infrastruktury:** klucz minisign, lokalny certyfikat podpisu (import do Trusted Root), osobne konto Windows dla usługi Brokera.

---

## 5. Dostawcy modeli, backendy agentowe i router

### 5.1 Dwa kontrakty
- **`ModelProvider`** (tokeny): `capabilities()`, `stream(req)` z anulowaniem, `health()`, `cost()`. Rodzaje: chat, STT, TTS, embeddings, wizja/OCR, S2S.
- **`AgentBackend`** (zadania): `submit_task`, `events`, `approve`, `steer`, `cancel`, `resume`. Router kieruje **zadania**.

### 5.2 Klasy
| Klasa | Kontrakt | Przykłady | Uwagi |
|---|---|---|---|
| **A. Lokalne** | ModelProvider | **wbudowany: llama.cpp** (Vulkan/CUDA/CPU); Ollama i LM Studio jako zewnętrzne endpointy przez adapter generyczny | **Bez kluczy API — domyślny i jedyny „mózg" (MVP)**; z kluczami — opcjonalny (komendy, fallback, offline) |
| **B. Chmura API** (**domyślna „mózg" zaawansowanej rozmowy**) | ModelProvider | Anthropic (`claude-opus-5-5`…), OpenAI (GPT-6 Sol/Astra/Luna — identyfikatory modeli pobierane automatycznie przez Models API w `accounts-hub`), **adapter generyczny** „endpoint zgodny z OpenAI/Anthropic" (Gemini, xAI Grok, DeepSeek, Kimi, Qwen/DashScope, Z.ai GLM, MiniMax, BytePlus, OpenRouter, Mistral…) | Klucze w Credential Manager, budżety; §5.5 |
| **C. Mosty** | AgentBackend | **Na start:** Claude Code, Codex CLI. **Potem:** Grok Build, Kimi Code, `agy` (eksperymentalnie) — §5.5 | §1.3, §8.5; zimny start (sekundy) → nie na ścieżce głosu |
| **D. Własne** | oba | HTTP/WebSocket/MCP | |

### 5.3 Uwagi do adaptera Anthropic (z aktualnej dokumentacji API)
Opus 5.5: **myślenia nie da się wyłączyć** (sterujemy `effort`, domyślnie `medium` — ustawiać jawnie); **wymuszony `tool_choice` `any/tool` = 400** (używać `auto` + `strict: true`); computer use tylko przez `computer_toolset_20260801`; streaming domyślnie; cache promptów (stabilny prefiks: narzędzia → system → wiadomości); obsługa `stop_reason: "refusal"` i serwerowe `fallbacks`; **bloki myślenia są związane z modelem i rozmową → historia ma być append-only** (to wpływa na barge-in, §6.5). Do weryfikacji w spike'u.

### 5.4 Router
Klasy zadań (głos-szybka, rozmowa, kod, planowanie, GUI/wizja, streszczanie, embeddingi) × ograniczenia (tag prywatności **i jurysdykcji**, budżet, opóźnienie, możliwości); fallback, circuit breaker; limity okien planów mostów wykrywane reaktywnie + estymata; uczenie z wyników (zmiany przez §12); tryb „Rada" (porównanie modeli, opcjonalny); **Mówczyni + Myślicielka** (§6.9). **Zaawansowana rozmowa domyślnie idzie przez API chmurowe**, lokalne lekkie modele (3–4B) tylko do komend, fallbacku i trybu offline.

### 5.5 Mosty i dostawcy rozszerzalni (Gemini, Grok, modele chińskie…) — stan z 29.09.2026
*„Grill" = **Grok** (xAI) — potwierdzone. Wiele stron dostawców było zablokowanych dla badania; regulaminy oznaczone [W] = z wtórnych cytatów → każda trasa przechodzi weryfikację przed włączeniem (karta zgodności, §1.3).*

**Krok 0 — adapter generyczny** (od Fali 1): pola `baseURL`, klucz, model ID, flagi możliwości (wizja, tools, kontekst), **tag prywatności i jurysdykcji** per profil. Pokrywa od razu: Gemini (endpoint OpenAI-compat), xAI (`api.x.ai/v1`), DeepSeek, Kimi (Open Platform i Kimi Code), Qwen/DashScope (klucz przypisany do regionu), Z.ai (OpenAI + Anthropic), MiniMax, BytePlus. Nie obsłuży: logowania subskrypcyjnego (device flow), zdarzeń CLI (plan, uprawnienia, wznawianie sesji), natywnych funkcji jak Gemini Live/computer use → do tego osobne mosty/moduły.

**Mosty CLI po Claude Code i Codex (kolejność):**
| # | Most | Dlaczego / ograniczenia | Status zgodności |
|---|---|---|---|
| 1 | **Grok Build** (`grok -p`, `--output-format streaming-json`, wznawianie sesji, ACP; Apache-2.0) | Najlepiej udokumentowany headless. Regulamin subskrypcji nie mówi wprost o sterowaniu z własnej aplikacji [?]; AUP zakazuje scrapingu/destylacji. **Domyślnie klucz API**, subskrypcja dopiero po weryfikacji | szary → API zielone |
| 2 | **Kimi Code CLI** (`kimi -p --output-format stream-json`, `kimi acp`; MIT) | Najprzyjaźniejszy regulamin wśród subskrypcji (osobisty użytek, nie modyfikować User-Agenta, jedna wspólna kwota) [W] | zielony/szary |
| 3 | **`agy` (Antigravity CLI, Gemini)** — *eksperymentalny* | Zapisy sprzeczne: ToS zabrania dostępu przez oprogramowanie trzecie i Google banowało za przechwytywanie OAuth, ale wg cytatu z forum uruchamianie oficjalnego `agy` jako procesu potomnego jest „supported workflow" [W]. Błąd: `agy -p` bez TTY zwraca pusty wynik. Do czasu wyjaśnienia **Gemini przez klucz API (adapter generyczny)** | szary |
| — | **Nie budować:** Qwen Coding/Token Plan (regulamin: „nie używać do skryptów i scenariuszy nieinteraktywnych") oraz GLM/ZCode (plan tylko w „oficjalnie wspieranych narzędziach", bany) | Te modele **wyłącznie przez API** | zabronione |

**Tagi prywatności/jurysdykcji** (wpisywane do rejestru, egzekwowane przez Router): Google konto osobiste — może trenować; Google klucz płatny/EOG — nie trenuje; xAI API — retencja 30 dni; **CN (DeepSeek, Kimi Moonshot) — dane w Chinach/Singapurze, może trenować**; SG (Z.ai, Alibaba Singapur); EU (Alibaba Frankfurt). Sesje z tagiem „prywatne" **nie** kierują ruchu do tras CN/„może trenować".

**Głos od dostawców (moduły `voice-*`):** Gemini Live (70 języków z PL; jakość PL do odsłuchu — zgłaszano regresję akcentu), xAI Voice Agent (zgodny z OpenAI Realtime; PL niepotwierdzony), **MiniMax Speech 2.8 (TTS z polskim i klonowaniem — kandydat do Voice Lab)**, **Qwen3-ASR (STT z polskim)**. Qwen3-TTS-Realtime, GLM-TTS, Aura-2 — bez polskiego.

**Model kosztowy:** ceny orientacyjne zmieniają się co tydzień (np. Qwen3.8-Flash ≈ $0,14/$0,42, GLM-5.3 ≈ $1,4/$4,4, DeepSeek Flash ≈ $0,14–0,30 wej.) → Router liczy koszt z tabeli cen w konfiguracji, nie z kodu.

### 5.6 Konta i klucze — dodawanie w dowolnym momencie (`accounts-hub`)
Nie masz teraz kluczy ani kont — **program działa bez nich** (profil A, lokalnie) i pozwala je **dodawać później, bez restartu i bez zmian w kodzie**:
- **Katalog dostawców** (`providers-catalog/*.toml`, deklaratywny, aktualizowalny): Anthropic, OpenAI, Google, xAI, DeepSeek, Kimi, Qwen, Z.ai, MiniMax, OpenRouter, Mistral, ElevenLabs, Cartesia, Azure, Soniox/Deepgram, „własny endpoint". Każdy wpis: baseURL, typ uwierzytelnienia, modele i możliwości, cennik, **tag prywatności/jurysdykcji**, link do regulaminu, status zgodności (§1.3).
- **Kreator „Dodaj dostawcę / konto / klucz"** (Ustawienia → Modele i dostawcy, także z pustego stanu i z podpowiedzi „dodaj klucz, aby odblokować X"): wybierz dostawcę → wklej klucz → **test połączenia** → automatyczne wykrycie modeli i możliwości (np. Models API) → przypisanie do klas zadań, agentek/ról i głosu (STT/TTS) → **limit kosztów per dostawca** (z możliwością wyłączenia, §14.6).
- **Mosty CLI:** kreator wykrywa zainstalowane CLI (Claude Code, Codex, później Grok Build/Kimi/agy), instaluje zależności i prowadzi do **logowania w wbudowanym terminalu ConPTY** (logujesz się Ty; Alfa nie widzi tokenów).
- **Bezpieczeństwo:** klucze tylko w Windows Credential Manager (per maszyna), nigdy w plikach konfiguracji ani w zwykłym eksporcie; rotacja/usuwanie jednym kliknięciem; import z zmiennych środowiskowych; redakcja w logach.
- **Bez klucza:** dostawca ma stan „nieskonfigurowany"; Router go pomija; funkcje zależne od niego pokazują, czego brakuje. Spike'i i testy chmurowe w F0 działają na atrapach/nagraniach, a prawdziwe wywołania włączają się, gdy dodasz klucz.
- **Nowy dostawca bez programowania:** wpis w katalogu + adapter generyczny (§5.5); dla nietypowych — agentka może przygotować wpis/adapter jako wtyczkę/umiejętność (R1–R2, §12.1).

---

## 6. System głosowy — Voice Suite (bardzo rozbudowany)

### 6.1 Cele
Rozmowa **pełnodupleksowa** jak z człowiekiem; polski jako język pierwszy (mieszany PL/EN); cztery odrębne głosy agentek; przerwanie w każdej chwili i zrozumienie korekty; działa lekko (CPU) i lepiej (GPU/chmura); każdy element wymienialny.

### 6.2 Moduły Voice Suite (każdy = trójka crate'ów, §3.2)
| Moduł | Rola | Domyślnie / kandydaci | Izolacja |
|---|---|---|---|
| `voice-audio` | wejście/wyjście, hot-plug, wybór urządzeń, mikser, ducking, normalizacja głośności, **routing per agentka** | crate `wasapi`, wątek RT | in-proc |
| `voice-dsp` | AEC, redukcja szumu, AGC | AEC z **własnym strumieniem TTS jako referencją** (crate `aec3`/`sonora`); awaryjnie loopback procesowy (Win10 20348+) lub tryb Communications; NS: RNNoise (DeepFilterNet3 opcja) | in-proc |
| `voice-vad` | wykrywanie mowy | Silero VAD (MIT) / TEN VAD | in-proc |
| `voice-turn` | koniec tury | **Smart Turn v3.2** (PL wśród 23 języków, ~8 MB, ~10 ms CPU) + polityka cierpliwości (hezytacje „yyy", regulowana) | in-proc |
| `voice-stt` | rozpoznawanie | whisper.cpp (large-v3 / turbo; CUDA/Vulkan/CPU), Parakeet v3 (ONNX), chmura (ElevenLabs Scribe v2 RT, Soniox, gpt-4o-transcribe); tryb dwuprzebiegowy (szybki partial + dokładny final), biasing/hotwords, auto PL/EN, pewność | proces |
| `voice-speaker` | **weryfikacja właściciela**, opcjonalnie diaryzacja | WeSpeaker/ECAPA przez sherpa-onnx; progi zależne od ryzyka akcji | in-proc |
| `voice-wake` | PTT, adresowanie, słowa wywoławcze | **v0 (F2): PTT / przełącznik (hook `WH_KEYBOARD_LL`, §7.3) + adresowanie po imieniu z transkryptu**; **v1 (F5):** opcjonalne słowa „Hej Alfa/Beta/Gama/Delta" (openWakeWord jest tylko angielski → własny trening na mowie syntetycznej + Twojej; włączane dopiero po spełnieniu FAR/FRR), tryb „zawsze słucham" z bramką właściciela, „nie przeszkadzać" | in-proc |
| `voice-dialog` | automat tur, barge-in, backchannel, intencje przerwań, prefiks usłyszany, wznawianie, mowa proaktywna | §6.5 | in-proc |
| `voice-tts` | synteza | lokalnie (baseline): **Pocket TTS + model PL społeczności** (CPU, RTF ≈ 0,21, ~200 ms do 1. fragmentu, klon głosu z krótkiej referencji), Piper pl_PL (zapas); *poza baseline (profil D):* Chatterbox Multilingual, XTTS-v2, F5 (wymagają CUDA / stabilnego ROCm), VibeVoice-Realtime (do sprawdzenia); chmura: ElevenLabs, Cartesia, Google Chirp 3 HD, Azure pl-PL, Gemini TTS, OpenAI, MiniMax Speech 2.8 (PL + klon); chunker, streaming, dostawca znaczników słów, cache fraz stałych, łańcuch fallback per agentka | proces |
| `voice-persona` | „biblia głosu" per agentka, styl→silnik, planista emocji, słownik wymowy, **normalizator PL**, podział mówione/ekranowe | §6.6–6.7 | in-proc |
| `voice-cmd` | szybka ścieżka komend bez LLM | „stop", „pauza", „głośniej/ciszej", „wycisz mikrofon", „przełącz na Deltę", „powtórz", „wznów", „anuluj" | in-proc |
| `voice-dictation` | **dyktowanie do dowolnej aplikacji** | STT → wstrzyknięcie tekstu (UIA/SendInput/schowek), komendy interpunkcji, profile per aplikacja | in-proc |
| `voice-readaloud` | **czytanie na głos** zaznaczenia/schowka/strony | UIA `TextPattern.GetSelection` (zapas: Ctrl+C), kolejka, tempo, podświetlanie | in-proc |
| `voice-lab` | benchmarki i **casting głosów** | opóźnienia p50/p95, echo, fałszywe przerwania, WER na korpusie własnym, dokładność prefiksu, ślepe A/B, proxy MOS, edytor słownika | proces |
| `voice-s2s` *(P1, opcjonalny)* | natywny speech-to-speech w chmurze jako „tryb szybkiej rozmowy" (zaawansowana rozmowa przez API) | OpenAI Realtime / Gemini Live (PL deklarowany; lokalnego S2S dla PL brak). Głosy = presety dostawcy przypisane do agentek (nie własne biblie głosu) — ocena w Voice Lab | proces |
| `voice-transcribe` *(P2)* | nagrania/spotkania z diaryzacją | pyannote community-1 (offline), komunikat o zgodzie osób trzecich | proces |

### 6.3 Profile potoku (A–D; profil wybierany **per maszyna** przez `device-profile`, §3.5; finalny wybór: Voice Lab)
| Profil | VAD / koniec tury | STT | „Mózg" rozmowy | TTS | Uwagi |
|---|---|---|---|---|---|
| **A. Minimum lokalne (musi działać na baseline)** | Silero + Smart Turn v3.2 (CPU) | whisper.cpp Vulkan, large-v3-turbo Q5_0 (547 MiB) z bramką VAD; fallback small Q5 / CPU | opcjonalnie lokalny 3–4B Q4 (llama.cpp Vulkan) do komend i fallbacku | **Pocket TTS PL** (CPU, RTF ≈ 0,21) — **v0: 4 wbudowane/zróżnicowane głosy (wysokość, tempo) bez kluczy; docelowo klon z referencji po castingu** / Piper pl_PL jako zapas | zero chmury; jakość rozmowy ograniczona rozmiarem LLM; opóźnienia best effort |
| **B. Hybrydowy (rekomendowany domyślny)** | jw. | lokalny (Vulkan) albo chmurowy | **chmurowy model przez API do zaawansowanej rozmowy** (Opus 5.5 / GPT-6 Sol / inne) | Pocket-PL lokalnie **albo** chmurowy (ElevenLabs/Cartesia) dla najlepszej jakości głosów | najlepszy kompromis; lokalne STT chroni audio |
| **C. Jakość-chmura** | jw. | Scribe v2 RT / Soniox | API | ElevenLabs / Cartesia; opcjonalnie tryb S2S (Realtime / Gemini Live) | najniższe opóźnienie i najlepszy PL; audio w chmurze (tag prywatności) |
| **D. Mocny lokalny** *(warianty według maszyny, §3.5)* | jw. | **D-AMD16 (desktop, RX 9070 XT 16 GB):** whisper.cpp Vulkan, large-v3 pełny. **D-CUDA (laptop, RTX 4050 6 GB):** whisper.cpp CUDA, turbo Q5 | **D-AMD16:** lokalny LLM 8–14B Q4 (llama.cpp Vulkan) i/lub API. **D-CUDA:** 3–4B lokalnie + API | **D-CUDA:** Chatterbox Multilingual / XTTS-v2 tylko jeśli mieszczą się w 6 GB obok STT (rezydencja na przemian) — inaczej Pocket-PL na CPU (i7-13700H). **D-AMD16:** Pocket-PL (CPU); Chatterbox/XTTS/F5 dopiero po dodatnim spike'u PyTorch-ROCm/portu Vulkan na RDNA4. *Przyszłe NVIDIA ≥ 12–16 GB: pełny lokalny stos* | wysoka jakość lokalnie tam, gdzie sprzęt pozwala; cele p50 ≤ 1300 ms, p95 ≤ 2000 ms *(do zmierzenia; Chatterbox ma zgłoszony akcent w PL — Voice Lab)* |

*Lokalnie odpada na baseline (AMD, bez CUDA):* Chatterbox, XTTS-v2, F5-TTS (PyTorch-ROCm/Windows niestabilny, RTF ≥ 1), duże LLM. *Ryzyka Vulkan/AMD:* zgłoszone crashe (`ErrorDeviceLost`, wczesne wyjście `whisper-stream`) na RDNA1/3/4 → przypięta wersja whisper.cpp + sterownik Adrenalin, automatyczny fallback na CPU (`-ng`), bramka VAD (turbo halucynuje na szumie). Parakeet v3 jest szybki na CPU, ale ma gorszy WER PL niż Whisper large-v3 (FLEURS-PL ok. 7,3% vs 4,7% wg źródła wtórnego).

*Uwaga jakościowa:* w polskim benchmarku (PUMA, wynik z wyszukiwarki, niezweryfikowany na PDF) Whisper-large-v3 dorównywał GPT-4o-Transcribe i wyprzedzał wiele nowych modeli → **nie zakładamy, że nowsze = lepsze po polsku**; rozstrzyga korpus własny. Kokoro, Orpheus, Kyutai, Dia, Sesame, Moonshine, Voxtral Realtime — bez polskiego (wg badania); Deepgram Aura-2 bez PL. Wszystkie oceny jakości TTS PL są **niezweryfikowane odsłuchem**.

### 6.4 Budżet opóźnień (pomiar: ostatnia ramka mowy wg VAD → pierwsza próbka na urządzeniu wyjściowym, mierzone loopbackiem)
| Etap | Lokalnie | Chmura |
|---|---|---|
| Koniec tury (Smart Turn + cierpliwość) | 250–500 ms | 250–500 ms |
| Finalizacja STT | 150–300 ms | ~150 ms |
| TTFT LLM | 200–600 ms | 200–500 ms |
| TTFB TTS | 200–500 ms | 75–300 ms |
| **Razem** | **~0,8–1,9 s** | **~0,7–1,5 s** |

**Cele (wstępne, do zmierzenia na baseline):** C. Jakość-chmura p50 ≤ 900 ms, p95 ≤ 1500 ms · B. Hybrydowy p50 ≤ 1300 ms, p95 ≤ 2000 ms · A. Minimum lokalne p50 ≤ 2000 ms, p95 ≤ 3000 ms (best effort, komunikowane w UI) · D. Mocny lokalny — jak B, mierzony osobno: D-AMD16 na desktopie, D-CUDA na laptopie. Cele A/B dotyczą baseline (emulowanego). Bluetooth +150–300 ms, HFP obniża jakość do 16 kHz → rekomendowany przewód/USB.

### 6.5 Dialog: przerywanie, korekty, rozumienie
Automat: `Idle → Listening → UserSpeaking → Thinking → Speaking → Interrupted → UserSpeaking…`

**Zatrzymanie dwustopniowe**
1. **Ducking** (−15 dB, < 50 ms) gdy VAD po AEC wykryje mowę podczas `Speaking`.
2. **Twardy stop** po potwierdzeniu (≥ 150–250 ms mowy, klasyfikacja ≠ backchannel): stop TTS, **anulowanie LLM**, czyszczenie kolejki. Realnie ≤ ~400 ms od początku wypowiedzi.
3. **Keyword-spotter „stop/czekaj"** (cel < 300 ms od początku słowa) w `voice-cmd`; „nie" przerywa tylko jako samodzielne słowo z pauzą przed i po, wyłącznie w stanie `Speaking` (inaczej fałszywe przerwania przy „nie no, dobrze") — mierzone w F2.
4. **Kill-switch** skrót/przycisk: < 200 ms, obsługiwany poza UI (§8.6).
5. Dwa zakresy: *Stop mowy* (Esc / „stop") ≠ *Stop wszystkiego* (kill-switch).

**„Usłyszany prefiks" — hierarchia:** (1) znaczniki słów z TTS (np. ElevenLabs); (2) forced alignment na wygenerowanym audio; (3) zliczanie odtworzonych próbek skorygowane o opóźnienie urządzenia (`GetStreamLatency`) → granica zdania, flaga `przybliżone`. Metryka: dokładność ±1 słowo.

**Historia append-only i gałęzie:** dziennik zdarzeń jest append-only; IR trzyma `assistant_full` i `assistant_heard_prefix` osobno; **nie edytujemy wcześniejszych tur** (edycja może unieważnić bloki myślenia Anthropic) — „edytuj" i „ponów" w UI tworzą **nowe gałęzie** (§14.8), każda gałąź to nieedytowana historia. Adapter decyduje o renderowaniu: Anthropic — pełna tura + dopisana notka „użytkownik usłyszał tylko: „…" i przerwał"; dostawcy z natywnym truncate (OpenAI Realtime `conversation.item.truncate`) — obcięcie po stronie dostawcy.

**Klasy intencji przerwania** (mały szybki model lub sam LLM) na wejściu `{usłyszany_prefiks, nie_powiedziane, wypowiedź}`: *korekta* · *uzupełnienie* · *pytanie doprecyzowujące* · *zmiana tematu* („wrócić do tego?") · *stop/anuluj* · *kontynuuj* (wznów od punktu cięcia). Backchannel („mhm", „tak") nie przerywa.

**Naturalność tur:** regulowana cierpliwość (dłużej czeka po „yyy"), backchannel agentki („aha", „rozumiem") w odpowiednich chwilach, fillery maskujące opóźnienie (poza prefiksem, przerywalne), **mowa proaktywna z etykietą** (nie w trakcie Twojej mowy, respektuje „nie przeszkadzać"), kolejka mówienia (jedna agentka naraz — `speaker` jako zasób wyłączny w Schedulerze).

**Przerwać można także pisząc** i **w trakcie działania narzędzi** (steering, §9.6).

### 6.6 Głosy agentek (casting)
- **Biblia głosu** per agentka: język, **postrzegany wiek: młoda dorosła 18–25**, barwa, rejestr, tempo, energia, zakres emocji, słowa-klucze do promptu głosu, pochodzenie i zgoda (brak klonowania prawdziwych osób; nie klonujemy lektorów z korpusów).
- **Szkic promptów** (do strojenia w Voice Lab; unikamy słów „girl/cute/child"):
  - *Alfa* — młoda dorosła kobieta, ok. 23 lat, ciepły spokojny środkowy rejestr, naturalne tempo, „uśmiech w głosie", czysta polszczyzna.
  - *Beta* — młoda dorosła kobieta, ok. 22 lat, pogodna, lekko wyższa i miękka barwa, bardzo wyraźna dykcja, umiarkowane tempo, życzliwa.
  - *Gama* — młoda dorosła kobieta, ok. 25 lat, niższy miękki rejestr, wolniejsze przemyślane tempo, rzeczowa.
  - *Delta* — młoda dorosła kobieta, ok. 20 lat, jaśniejsza barwa, żwawe tempo, energiczna i konkretna.
- **Ścieżka:** (1) cztery głosy zaprojektowane z opisu tekstowego w chmurze (ElevenLabs Voice Design / Gemini voice design — sprawdzić ToS użycia wyjścia jako referencji); (2) 15–30 s czystej referencji per agentka; (3) lokalny klon w **Pocket TTS PL** (CC-BY-4.0, CPU; jakość klonu mocno zależy od próbki referencyjnej) **albo** zostajemy w chmurze — Chatterbox/XTTS/F5 nie mieszczą się na baseline (na maszynach z NVIDIA — profil D); (4) odsłuch ślepy A/B.
- **Kontrole automatyczne (pomocnicze):** mediana F0, podobieństwo embeddingów mówcy między agentkami (muszą być wyraźnie różne), round-trip STT (WER/CER), UTMOS/TTSDS2 tylko porównawczo (UTMOS trenowany na angielskim). **Brak wiarygodnego automatu do „wieku głosu" → decyzja słuchowa człowieka.**
- Fallback: łańcuch silników per agentka; głos zapasowy nie może brzmieć jak inna agentka.

### 6.7 Jakość mówienia
Normalizator PL (liczby, daty, skróty, waluty, URL, kod) + słownik wymowy (edytowalny); **dwa kanały**: mówiony (krótki, bez markdown) i ekranowy (kod, tabele) — model streszcza głosem, szczegóły na ekranie; styl przez znaczniki mapowane na możliwości silnika (tempo, energia, emocja; tagi tam, gdzie silnik je ma); strumieniowanie zdanie-po-zdaniu bez przerw; 24–48 kHz; cache fraz stałych.

### 6.8 Akustyka
AEC krytyczny: referencja = własny strumień TTS (najdokładniejsza), loopback jako zapas; porównanie z trybem Communications Windows (uwaga: domyślnie tłumi inne strumienie o 80%). Autokalibracja opóźnienia pętli, hot-plug, konflikt z aplikacjami w trybie wyłącznym, słuchawki → agresywniejsze progi barge-in, adaptacja do szumu otoczenia (próg VAD, głośność), tryb szeptu (cichsza mowa).

### 6.9 Mówczyni + Myślicielka i narracja
Szybka agentka w roli Mówczyni (domyślnie Alfa) prowadzi rozmowę i **deleguje** ciężką pracę do agentek w innych rolach; przy długich zadaniach krótko raportuje postępy (gadatliwość w ustawieniach), pozwala się przerwać bez zabijania pracy w tle. **Narracja generowana ze zdarzeń magistrali**, nie z pamięci modelu; „gotowe" dopiero po weryfikacji agentki w roli Krytyczki (domyślnie Gama). Do **zaawansowanej rozmowy** Mówczyni używa modelu chmurowego przez API (§5.2), a lokalne lekkie modele obsługują szybkie komendy i fallback.

### 6.10 Bezpieczeństwo głosu
Pewność STT jest wejściem klasyfikatora ryzyka. **Weryfikacja właściciela** (`voice-speaker`) dla akcji ryzykownych. **Destrukcyjne akcje wywołane głosem** wymagają potwierdzenia **nie-głosem na każdym poziomie autonomii** (odczytane: „Usuwam 14 plików z X — potwierdź", odpowiedź kliknięciem/klawiszem w Broker-UI) — **reguła Jądra obowiązuje także na L4** (wyjątek tylko ręcznie w Broker-UI), bo błędne rozpoznanie mowy nie może skasować danych. Do czasu `voice-speaker` (F5) każda ryzykowna akcja zlecona głosem jest potwierdzana w Broker-UI fizycznym wejściem. Komendy z TV/YouTube/rozmowy obok nie mogą wyzwalać akcji (adresat: wake-phrase/PTT + właściciel).

### 6.11 Korpus własny i strojenie
Twoje nagrania (bramka ludzka #3) → zestaw testowy STT (cisza/szum/głośniki/słuchawki, PL + mieszane PL/EN, nazwy własne, komendy), pozytywy do słów wywoławczych, enrollment weryfikacji mówcy. Korpus trzymany lokalnie, poza gitem.

---

## 7. Dostęp do systemu i „computer use"

### 7.1 Wybór trasy per akcja
**API/CLI/COM > UI Automation > wizja > syntetyczne wejście**; degradacja przy błędzie; **weryfikacja po każdej akcji GUI**; ocena jakości drzewa UIA (liczba węzłów) decyduje o przełączeniu na wizję.

### 7.2 Katalog możliwości
| Obszar | Zakres |
|---|---|
| **Pliki i foldery** | odczyt/zapis/kopiowanie/przenoszenie/usuwanie (domyślnie do Kosza), katalogi, dowiązania, atrybuty, ACL, obserwatory zmian, wyszukiwanie (Windows Search + indeks), archiwa, hashe, udziały sieciowe, OneDrive |
| **Procesy i usługi** | procesy, usługi, zadania harmonogramu, autostart, WMI/CIM, liczniki, Dziennik zdarzeń |
| **System i ustawienia** | rejestr, env, zasilanie, wyświetlacze, audio (głośność/endpointy), Bluetooth, sieć/Wi-Fi, drukarki, zapora, hosts, winget/MSI/MSIX, Windows Update, sterowniki |
| **Powłoki** | PowerShell 7/5, cmd, **ConPTY**, **WSL2**, SSH, Git, Docker |
| **GUI** | UIA (drzewo, wzorce), zrzuty per monitor (DPI), OCR (Windows.Media.Ocr / Tesseract / PaddleOCR), mysz/klawiatura/dotyk/pióro (SendInput), schowek (formaty + historia), okna i pulpity wirtualne, drag&drop, nagrywanie ekranu, globalne skróty |
| **Aplikacje** | uruchamianie/zamykanie/instalacja, **Office COM** (Word/Excel — P1/F6; reszta P2), aplikacje Store, PDF, multimedia |
| **Przeglądarka** | CDP/Playwright, własny profil + rozszerzenie; sesje zalogowane za zgodą (z deny-listą z §1.3) |
| **Sieć** | HTTP/WebSocket, pobieranie, wyszukiwanie, poczta/kalendarz (MCP) |
| **Multimedia** | kamera/mikrofon, odtwarzanie, ffmpeg |
| **Automatyzacja** | harmonogram (świadomy blokady ekranu/uśpienia), wyzwalacze (plik, czas, skrót, fokus okna, schowek, e-mail), rejestrator makr → umiejętność (P2) |
| **Powłoka Windows** | toast, zasobnik, autostart, handler protokołu, „Wyślij do" |
| **Podniesione uprawnienia** | przez Broker, na żądanie z UAC (§8.4) |

### 7.3 Ograniczenia nie do obejścia („Czego agentka nie może")
UAC (bezpieczny pulpit), ekran blokady, Windows Hello, CAPTCHA — nieautomatyzowalne. **UIPI:** proces bez podniesienia nie steruje oknami elevated → osobny input-helper z `uiAccess=true` (wymaga podpisu — wystarczy własny certyfikat lokalny — i `Program Files`), objęty zakazem z §8.2. UIA daje puste drzewo w części Electron/Java/Qt/Delphi/DirectX/Citrix/RDP/SAP → macierz aplikacja × trasa w `evals/`. Okna chronione przed przechwyceniem → czarne klatki (wykrywanie). Focus-stealing prevention. **Push-to-talk globalny wymaga hooka klawiatury (`WH_KEYBOARD_LL`)**, bo zwykły skrót nie zgłasza puszczenia klawisza; hooki, PTT i dyktowanie (SendInput) **nie działają, gdy na pierwszym planie jest okno administratora** bez helpera `uiAccess` — UI to jasno komunikuje. Zadania z harmonogramu wymagają odblokowanej sesji i `prevent-sleep`.

### 7.4 Praca bez przejmowania myszy
Domyślnie **wzorce UIA/wejście w tle + wyłączność zasobu „ekran/mysz" z przekazywaniem sterowania** (lock w Schedulerze). Spike (≤ 1 tydz.): druga sesja (RDP loopback/osobne konto). VM/Windows Sandbox tylko jako piaskownica ryzykownego kodu.

### 7.5 Panel „Ekran"
Podgląd na żywo z nakładką (ramki UIA, ścieżka kursora, opis kroku) i **Przejmij / Pauza / Stop**.

---

## 8. Bezpieczeństwo i uprawnienia

### 8.0 Model zagrożeń (`docs/THREAT_MODEL.md`, F0)
**Aktywa:** pliki i sekrety, sesje przeglądarki, klucze API, pamięć długoterminowa, integralność Jądra i audytu, prywatność audio/ekranu. **Aktorzy:** niezaufana treść (web, mail, pliki, ekran, **dźwięk z TV/rozmowy**), złośliwy serwer/opis narzędzia MCP, zatruta wtyczka/umiejętność, inny proces użytkownika, halucynujący model, atak sieciowy (DNS rebinding), samo-modyfikujący się Ulepszacz. **Zasada „lethal trifecta":** dane prywatne + niezaufana treść + kanał wyjścia nie współistnieją bez potwierdzenia. Red-team z **progiem liczbowym** jako bramka.

### 8.1 Broker
Osobny proces/usługa na osobnym koncie: wydaje **tokeny zdolności** (`fs.read/write(zakres)`, `shell.exec`, `gui.control(aplikacja)`, `net.egress(host)`, `secrets.read`, `system.admin`; TTL; **potomek ≤ rodzic**), prowadzi zatwierdzenia, jest **jedynym writerem audytu** (pliki append-only przez ACL; głowa łańcucha hashy kotwiczona poza zasięgiem agentów), trzyma polityki Jądra, obsługuje kill-switch. Narzędzia wykonujące: restricted token / low-integrity / AppContainer dla ≤ L3.
**Jądro (agent nie zmienia):** silnik uprawnień, audyt, watchdog, aktualizator, kill-switch, tagi prywatności, budżety, egress-allowlista, klasyfikator ryzyka, polityka taint, bramka ewaluacyjna, deny-listy z §1.3.

### 8.2 Kanał zatwierdzeń
Zakaz `gui.control` wobec procesów Alfy, Brokera i helpera; okno zatwierdzeń prowadzi **Broker-UI** — osobny mały natywny proces w Twojej sesji, uruchomiony na **wyższym poziomie integralności** niż agentki (UIPI blokuje wtedy wstrzykiwanie wejścia z procesów agentek); sama usługa Brokera (logika, tokeny, audyt) działa w tle i nie ma UI (usługi w sesji 0 nie mogą pokazać okna). Nie WebView z treścią LLM; zatwierdzenia tylko z wejścia niewstrzykniętego (fizyczne kliknięcie/klawisz); **Windows Hello — opcjonalnie** (włączane w Ustawieniach) przy L4, akcjach admina i zmianach polityk Jądra; UI: minimalne capabilities Tauri per okno, ścisłe CSP bez inline i Trusted Types; **markdown z LLM renderowany do HTML w Rust** (pulldown-cmark + sanitizacja `ammonia`, bez surowego HTML i skryptów) i wstawiany do dokumentu — dzięki temu działa wirtualizacja, zaznaczanie tekstu, `Ctrl+F` i czytniki ekranu; **sandboxowany iframe bez IPC tylko dla aktywnych artefaktów HTML/SVG** (jeden na podgląd). Port debugowania WebView2 (CDP) istnieje **wyłącznie w buildzie testowym**; test CI sprawdza, że w produkcji jest zamknięty.

### 8.3 Poziomy autonomii (prosto: „jak bardzo agentka działa sama")
| Poziom | Nazwa | Co to znaczy w praktyce |
|---|---|---|
| L0 | Podgląd | Agentka tylko czyta i podpowiada, nic nie zmienia |
| L1 | Pytaj o wszystko | Każda zmiana wymaga Twojego „tak" |
| L2 | Pytaj o ryzykowne | Drobiazgi robi sama; pyta przy usuwaniu, wysyłaniu danych na zewnątrz, instalacji |
| **L3** | **Bardzo wysoka (domyślny na start)** | Działa sama w całym Twoim profilu i we wskazanych aplikacjach; pyta tylko przy rzeczach nieodwracalnych poza zakresem albo gdy działa na podstawie niezaufanej treści (mail, strona, plik) |
| **L4** | **Maks** | Nie pyta o nic poza twardymi blokadami Jądra (§8.1) i potwierdzeniem destrukcji zleconej głosem (§6.10). Włączany **jednym przełącznikiem w Ustawieniach** (globalnie, per sesja lub per agentka; opcjonalnie na czas) |

- **Zmieniasz w dowolnym momencie** w Ustawieniach → Uprawnienia, głosem („Delta, pracuj na Maksie") lub w pasku sesji. Poziom jest per sesja i per agentka; agentka **nie może** podnieść własnego poziomu — robi to Broker po Twoim potwierdzeniu.
- **O co chodzi z „Hello":** to tylko *opcjonalny* sposób potwierdzenia przy podnoszeniu do L4 (PIN/odcisk Windows). Sens: żeby agentka albo złośliwy tekst na stronie nie podniosły sobie uprawnień sami. Domyślnie wystarczy kliknięcie w oknie Brokera (fizyczne wejście, którego agentka nie potrafi wstrzyknąć); Hello można włączyć lub wyłączyć w Ustawieniach.
- **Nawet na L4 zostają:** kill-switch, dziennik cofania, audyt i kilka twardych blokad (np. wyłączenie audytu, formatowanie dysku systemowego). To nie ogranicza możliwości — chroni Cię przed skutkami błędu modelu lub wstrzykniętego polecenia.
- Klasyfikator ryzyka: odwracalność, zakres, wpływ zewnętrzny, destrukcyjność, pewność STT.

### 8.4 Podniesienie uprawnień
Broker uruchamia operacje elewowane **na żądanie przez UAC** (lub, opt-in, usługa z krótką allowlistą poleceń, weryfikacją wywołującego Authenticode+SID i zgodą per eskalacja). Brak otwartej dla każdego procesu usługi admina.

### 8.5 Mosty CLI jako „opaque worker"
Praca w worktree/kopii; natywne uprawnienia CLI = „ask" przekierowane do naszego kanału (Claude Code: `--permission-prompt-tool`; Codex: approvals app-server — do potwierdzenia w spike); przez MCP wystawiamy tylko narzędzia specyficzne dla Windows (F4: schowek, okna; F6: UIA, zrzuty, rejestr), bez fs/shell; cofanie = snapshot przed/po; pamięć tylko przez `recall`; zdarzenia CLI w audycie oznaczone „niezależnie niezweryfikowane".

### 8.6 Kill-switch
Skrót globalny (konfigurowalny; domyślnie **`Ctrl+Shift+F12`** — `Pause` nie ma na laptopach, a na polskiej klawiaturze `Ctrl+Alt` = AltGr, więc np. `Ctrl+Alt+Shift+S` to wielka litera „Ś"; **reguła: żaden skrót globalny `Ctrl+Alt(+Shift)` z literami a, c, e, l, n, o, s, x, z** — test w CI), przycisk w kapsule, zasobnik, „stop" głosem. **Skrót/przycisk obsługuje watchdog/broker, nie UI** (< 200 ms); zabijanie drzew procesów przez Job Objects.

### 8.7 Pozostałe
- **Sesja `tainted`** po pierwszym niezaufanym wejściu: wysokie ryzyko i `net.egress` wymagają potwierdzenia. Wzorzec dwóch LLM tam, gdzie agent nie musi widzieć treści.
- **Pamięć z proweniencją:** wpisy z niezaufanej treści nie awansują do „globalnej"; auto-`remember` z niezaufanej treści wyłączony.
- **MCP/wtyczki:** hash opisów narzędzi, poziomy zaufania; serwer MCP Alfy **tylko stdio-proxy lub named pipe z ACL na SID**, token sesyjny z TTL, bez nasłuchu TCP.
- **Odwracalność:** flaga `reversible: yes|scoped|no` w manifeście narzędzia; shell w zakresie ze snapshotem (pre-image/shadow-git), poza zakresem — potwierdzenie; dziennik cofania dla `fs.*`, VSS tylko masowo.
- **Sekrety:** Credential Manager/DPAPI (chroni przed kradzieżą offline, nie przed procesem tego samego użytkownika); redakcja w logach; pola haseł (UIA `IsPassword`) i deny-lista aplikacji/okien/URL wykluczone ze zrzutów/OCR.
- **Uczciwie o L4:** bez izolacji, audyt best-effort.
- **Łańcuch dostaw:** `cargo deny/vet`, lockfile'e, SBOM; modele tylko safetensors/ONNX/GGUF z hashami; aktualizacje podpisane minisign własnym kluczem.

---

## 9. Agentki i orkiestracja

### 9.1 Runtime
Pętla plan → działanie → obserwacja → weryfikacja; event-sourced; **checkpointy**; budżety (tokeny/czas/$); narzędzia równoległe; detektor pętli.

### 9.2 Cztery agentki na start = persony; role są osobno i zawsze zmienne
**Persona** (stała tożsamość, zawsze żeńska): imię + głos + charakter + fraza + subtelny kolor w UI. **Rola** (zmienna): zestaw zadań, narzędzi i uprawnień w danej sesji. Dzięki temu możesz w każdej chwili ustalić, kto co robi.

| Persona | Charakter | Głos (szkic, §6.6) | Fraza | Domyślna rola w obsadzie „Standard" |
|---|---|---|---|---|
| **Alfa** | ciepła, spokojna, konkretna | ok. 23 l., ciepły środkowy rejestr | „Hej Alfa" | Dyrygentka + Mówczyni (prowadzi rozmowę, deleguje) |
| **Beta** | pogodna, uporządkowana, troskliwa | ok. 22 l., lekko wyższa, miękka, wyraźna dykcja | „Hej Beta" | Strażniczka pamięci i organizacji + Pisarka/Tłumaczka (plany dnia, dokumenty, poczta) |
| **Gama** | rzeczowa, dociekliwa, wolniejsza | ok. 25 l., niższy miękki rejestr, wolne tempo | „Hej Gama" | Badaczka + Krytyczka/Weryfikatorka + Myślicielka |
| **Delta** | energiczna, zwięzła, praktyczna | ok. 20 l., jaśniejsza barwa, żwawe tempo | „Hej Delta" | Wykonawczyni/Operatorka komputera + Koderka |

**Obsada ról** (ustawiana per sesja / zadanie / szablon):
- Katalog ról: Dyrygentka, Mówczyni, Myślicielka/Planistka, Wykonawczyni/Operatorka, Koderka, Krytyczka/Weryfikatorka, Badaczka, Strażniczka pamięci/organizacji, Pisarka/Tłumaczka (+ własne z Kreatora).
- **Zmiana obsady:** Ustawienia → Agentki → Obsada, pasek sesji, głosem („Beta, teraz ty prowadzisz", „Delta, przejmij weryfikację") albo przez Marszałka na Twoje polecenie. Zmiana jest natychmiastowa i pozostaje w dzienniku.
- **Szablony obsad:** *Standard* (jak wyżej) · *Solo* (jedna agentka wszystkie role — najlżejsza) · *Kodowanie* (Delta prowadzi, Gama recenzuje, Beta notuje) · *Badania* · własne.
- **Uprawnienia idą za rolą, nie za personą**, pod sufitem sesji (§8.3): przy zmianie obsady Broker wydaje nowe tokeny; Krytyczka ma tylko odczyt, Badaczka pracuje na niezaufanych źródłach w izolacji, Wykonawczyni ma narzędzia systemowe. **Głos idzie za personą** — agentka mówi zawsze swoim głosem, cokolwiek robi.
- Kto odpowiada, gdy nie zwracasz się po imieniu: agentka z rolą Dyrygentki. Zwrot po imieniu zawsze wygrywa.

**Zasady wspólne**
- **Język żeński** w promptach i mowie („zrobiłam", „sprawdziłam"); mówią wyłącznie po jednej naraz; przekazanie głosowe („Przekazuję Delcie…").
- **Usługi systemowe bez persony i głosu:** Scheduler (deterministyczny), Marszałek (LLM-tłumacz reguł), Diagnosta, Ulepszacz, Watchdog, Router/Koszty (deterministyczny). Ich raporty relayuje agentka w roli Dyrygentki.
- **Więcej agentek** dodajesz **Kreatorem** (§9.5) z szablonów agentek (persona + głos + rola); agent podsystemu = paczka narzędzi + skill, nie stale aktywna pętla LLM.

### 9.3 Scheduler i Lock Manager (deterministyczne, w rdzeniu)
Priorytety, DAG zależności, **zasoby wyłączne** (mysz/klawiatura/ekran, mikrofon, **głośnik/mówienie**, wskazane pliki), limity współbieżności, timeouty, wykrywanie cykli i zakleszczeń, okna czasowe, budżety.

### 9.4 Marszałek — tłumacz reguł
Nie szereguje; zamienia polecenia w języku naturalnym na **deklaratywne reguły**, proponuje kolejność, wykrywa konflikty; Ty zatwierdzasz. **Reguły tylko zawężają uprawnienia.** Pauza tylko w punktach atomowych i tylko agentów GUI/audio.
```yaml
id: gui-exclusive
when: { resource: screen_input }
then: { mode: exclusive, queue: priority, max_wait: 120s, on_timeout: ask_user }
---
id: voice-first
when: { event: user_speaks, confidence: ">=confirmed" }
then: { preempt: [narration], pause_at_atomic: [agent:delta-gui-*], resume_after: turn_end }
```

### 9.5 Kreator agentów
Opis słowami → **manifest** → test w piaskownicy → biblioteka. Umiejętności = playbooki + narzędzia; import MCP (z hashowaniem opisów).
```yaml
id: porzadkowanie-pobranych
role: "Sortuje folder Pobrane wg typu i daty"
persona: delta
model_policy: { class: cheap-tool-use, privacy: local_only }
tools: [fs.list, fs.move, fs.mkdir]
permissions: { fs.write: ["%USERPROFILE%\\Downloads\\**"], level: L3 }
memory: { scope: agent, retain: 30d }
triggers: [{ cron: "0 20 * * 0" }]
budget: { tokens: 200k, wall: 15m }
```

### 9.6 Sterowanie w locie (steering)
Kolejka sterująca per przebieg: nowa wiadomość/korekta (tekst lub głos) trafia do agentki **między atomowymi krokami**; ona przeplanowuje. „Pauza po bieżącej akcji", „cofnij ostatni krok", „zmień cel".

### 9.7 Serwer MCP Alfy
Alfa jest klientem MCP i serwerem MCP (narzędzia Windows dla mostów CLI; zabezpieczenia §8.7). Wzorce wieloagentowe: sekwencja, fan-out, pętla weryfikatora (debata/A2A — P2).

---

## 10. Pamięć

| Warstwa | Zawartość | Magazyn |
|---|---|---|
| **Robocza** | okno kontekstu sesji, przypięte fakty, kompaktowanie | RAM + checkpoint |
| **Epizodyczna** | zdarzenia sesji + streszczenia | SQLite |
| **Semantyczna** | fakty, preferencje, encje/relacje z proweniencją, pewnością, TTL | SQLite (szyfrowane) + sqlite-vec + FTS5 |
| **Proceduralna** | wyuczone umiejętności | pliki + indeks |

- **Zakresy:** `sesja` (domyślny — **każdy czat ma osobną pamięć**) · `projekt` · `globalna` · `agentka`. Dzielenie tylko jawnie.
- `remember / recall / forget`; ekstrakcja z zatwierdzeniem lub automatyczna (nie z treści niezaufanej); **`forget` kaskadowo** (embeddingi, streszczenia, kopie, eksporty).
- **Nocna konsolidacja** (modele lokalne; nie startuje na baterii ani w trybie gry/pełnego ekranu — dotyczy też Ulepszacza): streszczanie, deduplikacja, sprzeczności, wygaszanie, awans za zgodą.
- **Inspektor pamięci** w UI; embeddingi lokalne wielojęzyczne (jakość PL do weryfikacji); szyfrowanie w spoczynku.
- Wyszukiwanie między sesjami = funkcja UI jako **Ty**, nigdy narzędzie agentki.

---

## 11. Sesje, okna czatu i pliki

- **Sesja:** `id, tytuł, polityka modeli, aktywne agentki, zakres pamięci, katalog roboczy, profil uprawnień, tag prywatności, załączniki, artefakty`. Szablony: Kodowanie, Research, Asystent głosowy, Administracja PC, Pusty.
- **Wiele okien czatu:** karty + widok dzielony + odłączane okna; praca równoległa w tle z powiadomieniami; fork; przypinanie; eksport; **przekazanie kontekstu** między czatami jako jawna wiadomość.
- **Pliki — wejście:** drag&drop, wklejanie, zrzut ekranu, montowanie folderu, „Otwórz w Alfie" z Eksploratora.
- **Pliki — oddawanie (panel Artefakty):** karta pliku (nazwa, ścieżka, rozmiar, podgląd/diff, wersje) + **Otwórz · Pokaż w Eksploratorze · Kopiuj (jako plik do schowka) · Zapisz jako… · Spakuj · Wyślij (MCP) · Przekaż do innego czatu** + przeciągnięcie do Eksploratora. Domyślnie `%USERPROFILE%\Alfa\Sesje\<nazwa>\out`.

---

## 12. Samonaprawa i samodoskonalenie

### 12.1 Pierścienie zmian
| Pierścień | Zmiana | Polityka |
|---|---|---|
| **R0** | prompty, słownik wymowy, ustawienia niekrytyczne | auto tylko dla zmian zawężających/bezpiecznych, po bramce ewaluacyjnej; cofalne |
| **R1** | umiejętności, manifesty agentek | po testach, przegląd w kolejce |
| **R2** | wtyczki Wasm (§3.2) | 1 klik po testach |
| **R3** | kod rdzenia | **poza v1** — tylko jako patch/issue dla sesji deweloperskiej (§4) |
| **Jądro** | §8.1 | agent nie zmienia |

Zatwierdzenia R1–R2 podpisywane kluczem chronionym TPM (opcjonalnie z Windows Hello). Diagnosta u Ciebie naprawia **konfigurację, skille i wtyczki**, nie kod rdzenia.

### 12.2 Samonaprawa programu
Watchdog (heartbeat, restart modułów, **safe-mode** po N awariach, rollback do ostatniej dobrej wersji/konfiguracji) → **Diagnosta** (klasteryzacja błędów → hipoteza → odtworzenie w piaskownicy → poprawka → testy+eval → **Propozycja zmiany**: diff, uzasadnienie, ryzyko, plan cofnięcia). Limity częstotliwości, okres wychładzania.

### 12.3 Samonaprawa zadań
Klasyfikacja przyczyny (środowisko, uprawnienia, narzędzie, model, zmiana UI) → **alternatywna trasa** → weryfikacja → zwięzły raport (co próbowano, co blokuje).

### 12.4 Samodoskonalenie (z ochroną przed Goodhartem)
Sygnały: kciuki, poprawki, **przerwania** (odfiltrowane po pewności AEC), czasy, koszty, porażki. Ulepszacz (idle/noc, modele lokalne): retrospektywy → propozycje promptów, nowych umiejętności z powtarzalnych przepływów, wag routera, słownika, sugestii ustawień. **Bramka ewaluacyjna w Jądrze** z **ukrytym holdoutem** niedostępnym dla Ulepszacza; walidacja **replayem offline na własnych logach**; N ≥ 5 powtórzeń. Ulepszacz **nie może** zmieniać tagów prywatności, budżetów, uprawnień, egress-allowlisty ani progów bramki. Panel **„Zdrowie systemu"**.

---

## 13. Logi, postępy, obserwowalność

**Zdarzenie:** `{id, ts, sesja, agentka, przebieg, span, rodzaj, poziom, payload_ref, koszt, hash_prev}` — append-only NDJSON + indeks SQLite; payloady szyfrowane kluczem per sesja (crypto-shredding → usuwalność bez łamania łańcucha). **Writer: Broker zapisuje tylko strumień Audyt** (z łańcuchem hashy po digestach); pozostałe strumienie (tokeny, GUI, głos, diagnostyka) zapisuje `core-log` w jądrze — Broker nie jest wąskim gardłem; wersjonowane schematy + upcastery + testy migracji.

| Strumień | Zawartość |
|---|---|
| **Audyt** | akcje wrażliwe, decyzje uprawnień, zmiany konfiguracji i samo-zmiany; zdarzenia mostów „niezależnie niezweryfikowane" |
| **Wywołania modeli** | dostawca, model, tokeny, koszt, opóźnienie, prompt/odpowiedź (redakcja przełączalna) |
| **Narzędzia i GUI** | krok + zrzut + migawka UIA — retencja domyślnie **7 dni**, szyfrowane, z deny-listą |
| **Głos** | opóźnienia etapów p50/p95, fałszywe przerwania, dokładność prefiksu, echo, WER na korpusie własnym |
| **Diagnostyka** | błędy, moduły, watchdog |

**Postępy:** drzewo *Plan → kroki → status* z %, ETA, akcją, kosztem; **kapsuła aktywności**; powiadomienia; **Oś czasu/Replay** krok po kroku; paczka diagnostyczna; pulpit kosztów/opóźnień/skuteczności/limitów planów. Poziomy TRACE…AUDIT; limity dysku.
**Prywatność:** macierz „dane → dostawca" egzekwowana tagiem sesji; ekran „co poszło do chmury"; nagrywanie osób trzecich (np. Teams) wymaga ostrożności także przy użytku osobistym.

---

## 14. UI / UX

**Kierunek wizualny:** „spokojny, jasny, szybki" — nowoczesny styl Windows 11 (Fluent 2), ale ciszej: dużo powietrza, jedna kolumna tekstu, cienkie podziały zamiast ramek, **jeden akcent koloru na raz** (kolor mówiącej/pracującej agentki), zero ciężkich gradientów i efektów w treści. Ładne przez typografię, rytm odstępów i płynny, krótki ruch, a nie przez dekoracje.

### 14.1 Trzy warstwy
1. **Powierzchnia (zawsze):** rozmowa, jedno pole wejścia (composer), przycisk mikrofonu, kapsuła aktywności.
2. **Panele (chowane, pamiętane per sesja):** lewy — **Sesje** (lista, wyszukiwanie, projekty); prawy — jeden panel naraz w kartach: Agentki · Oś czasu/Logi · Pliki/Artefakty · Pamięć · Ekran · Głos; każdy panel można **odłączyć do osobnego okna**. **Tryb skupienia** (F11 lub `Ctrl+Shift+Enter`) chowa wszystko poza rozmową.
3. **Ustawienia i konfiguracja** (§15) — osobny widok z drzewem i wyszukiwarką.

### 14.2 Układ i szkic
```
┌─ Alfa ──────────────────────────────────────────────── ─ □ × ┐  pasek tytułu 32–40 px (stały, Mica)
│ ☰  Projekt X ▸ Raport Q3 ▾      (A)(B)(Γ)(Δ)    ● Hybryda · L3 · 2,14 zł  ⚙ │
├────────────┬──────────────────────────────────────┬──────────────────┤
│ SESJE  ⌕   │                                      │ Agentki│Oś│Pliki │  panel prawy 300–520 px
│ ● Raport Q3│   rozmowa — kolumna ~72 znaki        │                  │  (zmienna szerokość,
│   Kod: API │   (≈560 px; wariant szeroki 760 px), │  (zawartość      │   przeciągana krawędź)
│   Zakupy   │   wyśrodkowana, dużo oddechu         │   panelu)        │
│ + Nowa     │  ╭ Δ Delta · edytuję raport.docx ─╮  │                  │
│            │  │ krok 3/7 ▰▰▰▱▱▱▱  0:42   ■ Stop │  │                  │
│ 240 px     │  ╰────────────────────────────────╯  │                  │
├────────────┴──────────────────────────────────────┴──────────────────┤
│  📎  Napisz… (@agentka, /komenda)        [Alfa ▾] [Hybryda ▾]   🎙  ➤ │  composer: 1–12 linii
└──────────────────────────────────────────────────────────────────────┘
```
- **Pasek górny (stały, cienki; auto-ukrywany tylko w trybie skupienia — inaczej psuje przeciąganie okna i Snap Layouts):** przełącznik lewego panelu, ścieżka projekt ▸ sesja (klik = zmień nazwę), **obsada agentek** jako 4 małe awatary (kółko z **greckim glifem α β γ δ** i kolorowym pierścieniem; świeci ta, która mówi lub pracuje; klik = szczegóły i zmiana roli), stan (profil głosu/modelu, poziom autonomii, koszt sesji — klik otwiera szczegóły), ustawienia. Własny pasek tytułu z natywnymi przyciskami okna i obsługą **Snap Layouts** Windows 11 (do sprawdzenia w F0 dla Tauri).
- **Responsywność (ważne dla laptopa 1080p ze skalowaniem 125–150%):**
  | Szerokość okna (px efektywne) | Zachowanie |
  |---|---|
  | ≥ 1440 | lewy i prawy panel zadokowane jednocześnie |
  | 1100–1440 | jeden panel zadokowany, drugi jako wysuwana szuflada |
  | 720–1100 | panele jako szuflady nad treścią; kolumna rozmowy pełna |
  | < 720 (min. okna 400 × 500) | tryb kompaktowy: tylko rozmowa + composer; panele jako arkusze pełnoekranowe |
- Szerokości paneli (lewy 240–320 px, zwijany do paska ikon 48 px; prawy 300–520 px), pozycja okna i monitor są **zapamiętywane per maszyna** (nakładka §3.5); **które panele są otwarte** — per sesja. Kolumna czytania max ~760 px (wąska/szeroka do wyboru).
- **Okno główne a zasobnik:** zamknięcie ukrywa okno (WebView żyje) przez N minut (domyślnie 10), potem WebView jest niszczony dla oszczędności RAM. Budżet: otwarcie z zasobnika **ciepłe ≤ 150 ms, zimne ≤ 1 s**.
- **Wiele okien** (odłączone panele, Szybkie pytanie, pigułka) korzysta ze **wspólnego środowiska WebView2** (jeden proces przeglądarki); koszt „+MB na okno" mierzony w F0(f); limit odłączonych okien z ostrzeżeniem.

### 14.3 Język wizualny — tokeny (w `packages/ui-kit`, jedno źródło prawdy, wartości do strojenia na makietach)
| Token | Wartość startowa |
|---|---|
| **Fonty** | systemowe (zero pobierania): **Segoe UI Variable** (tekst), **Cascadia Mono** (kod); fallback `system-ui` |
| **Skala typografii** | 12 · 13 · **14 (baza)** · 16 · 20 · 24 · 32 px; interlinia 1,5 (tekst), 1,25 (nagłówki); grubości 400 / 600 |
| **Siatka odstępów** | 4 px: 4 · 8 · 12 · 16 · 24 · 32 · 48 |
| **Promienie** | 6 (kontrolki) · 10 (karty, wiadomości) · 16 (nakładki, pigułka głosowa) · pełny (awatary, chipy) |
| **Elewacja** | 3 poziomy; w jasnym: delikatny cień + 1 px obramowanie; w ciemnym: tylko jaśniejsze tło + obramowanie (bez cieni) |
| **Neutralne** | chłodna szarość (≈ slate), tło jasne ~#FAFAFB / ciemne ~#131417; tekst główny kontrast ≥ 7:1, pomocniczy ≥ 4,5:1 |
| **Kolory agentek** (akcenty, nie tekst) | **Alfa** — ciepły koral · **Beta** — miętowa zieleń · **Gama** — indygo · **Delta** — lazur/turkus (propozycja; ostateczne odcienie z makiet, sprawdzane skryptem kontrastu i symulacją deuteranopii; odrębne od kolorów błąd/ostrzeżenie/sukces); każdy w wariancie jasnym i ciemnym, kontrast ≥ 3:1 dla elementów UI (WCAG 1.4.11) i ≥ 4,5:1, gdy kolor niesie tekst; zawsze w parze z glifem α/β/γ/δ i imieniem (nie tylko kolor) |
| **Semantyczne** | sukces / ostrzeżenie / błąd / informacja + **kolor ryzyka** w kartach zatwierdzeń (niskie/średnie/wysokie) |
| **Ruch** | 120 ms (hover, naciśnięcie) · 180 ms (panele, szuflady) · 240 ms (orb, tryb głosowy); ease-out; **tylko `transform`/`opacity`**; `prefers-reduced-motion` i ustawienie „bez animacji" wyłączają ruch |
| **Ikony** | jeden zestaw liniowy (np. Lucide) jako SVG inline, tylko użyte ikony (tree-shaking), 16/20 px, linia 1,5 px |
| **Tła okna** | **natywna Mica** (efekt okna Windows 11, liczony przez system — tani) dla paska i paneli; treść na jednolitym, nieprzezroczystym tle dla czytelności; CSS `backdrop-filter` zabroniony poza jedną nakładką (paleta `Ctrl+K`); fallback na jednolity kolor przy: wyłączonej przezroczystości w Windows, trybie baterii, trybie wysokiego kontrastu |
| **Motywy** | jasny / ciemny / auto (za Windows), wysoki kontrast (`forced-colors`), akcent: kolor agentki albo kolor akcentu Windows; gęstość: komfortowa / kompaktowa; zoom 80–200% (`Ctrl+=`/`Ctrl+-`/`Ctrl+0`) |

**Dostępność (od F1):** klawiatura-first (każda akcja osiągalna bez myszy, widoczny fokus 2 px), WCAG 2.2 AA, `aria-live` z throttlingiem dla strumienia (ogłaszanie na końcu zdania), karta potwierdzenia jako `alertdialog` z pułapką fokusu, cele kliknięcia ≥ 24 px, napisy do mowy, Storybook + regresja wizualna + axe w CI, PL/EN.

### 14.4 Stany systemowe
Offline (fallback lokalny + baner + kolejka) · 429/wyczerpane okno planu (kiedy się odnowi) · brak zgody mikrofonu (`ms-settings:privacy-microphone`) · GPU OOM · przerwane pobieranie (wznawianie) · brak miejsca · crash-loop modułu · pętla agentki · zablokowany ekran · zmiana urządzenia audio.

### 14.5 Onboarding (w MVP)
Test mikrofonu → wybór profilu głosu (§6.3) → pomiar sprzętu (`device-profile`) → **konta i klucze: „dodaj teraz" albo „pomiń — dodam później"** (§5.6; bez kluczy startuje profil lokalny) → tryb próbny i prosty opis poziomów autonomii (start L3, zmiana jednym przełącznikiem) → opcjonalne nagranie korpusu własnego → opcjonalny import paczki `.alfa` z innej maszyny (§15.1). **Krok „mosty CLI"** (wykrycie Claude Code/Codex, zależności, logowanie w wbudowanym terminalu ConPTY — logujesz się sam, nie przechwytujemy tokenów) pojawia się w onboardingu od Fali 4; wcześniej ten krok jest niewidoczny.

### 14.6 Koszty i zmęczenie zatwierdzeniami
**Limit miesięczny w PLN ustawiany w aplikacji — z możliwością całkowitego wyłączenia** (po wyłączeniu zostaje tylko wskaźnik zużycia i opcjonalne alerty); ceny dostawców są w USD → przeliczanie kursem NBP (tabela A, odświeżany raz dziennie, kurs zapasowy w konfiguracji); szacunek przed długim zadaniem; osobny budżet tła (domyślnie tylko modele lokalne); wskaźnik zużycia okna planu (best effort). Zatwierdzenia: **„plan do zatwierdzenia" zamiast 40 pytań**, szablony uprawnień, metryka pytań/godz.; „zawsze zezwalaj w tym zakresie" nie eskaluje do L4.

### 14.7 Budżety wydajności UI (mierzone w CI przez ślad CDP/Playwright na szkielecie, pod limitami baseline)
| Wskaźnik | Cel |
|---|---|
| Pisanie w composerze (klawisz → widoczny znak) | ≤ 16 ms p95 |
| Strumień odpowiedzi 100 tokenów/s | płynne 60 kl./s; **aktualizacja DOM najwyżej raz na klatkę** (bufor + `requestAnimationFrame`); zadania na głównym wątku ≤ 50 ms |
| Otwarcie panelu / szuflady | pierwsza klatka ≤ 100 ms |
| Przełączenie sesji (1000 wiadomości) | ≤ 150 ms (lista wirtualizowana — renderowane tylko widoczne wiadomości ± 1 ekran) |
| Paleta poleceń (`Ctrl+K`) | otwarcie ≤ 50 ms, wyniki ≤ 16 ms na znak |
| Paczka JS przy starcie | ≤ 150 KB gzip; panele, ustawienia, Voice Lab, edytor surowy jako leniwie ładowane moduły |
| CSS | ≤ 30 KB gzip; zero fontów webowych; bez CSS `backdrop-filter` w treści (dozwolone najwyżej na 1 małej nakładce) |
| Bezczynność z otwartym oknem | ~0% CPU; animacja orba zatrzymana, gdy okno ukryte/zminimalizowane; wskaźniki głośności ≤ 30 kl./s |
| Markdown i kod | przyrostowe parsowanie (zamknięte bloki się nie przerenderowują); **podświetlanie składni leniwie i w Web Workerze** po zakończeniu bloku; długie wyniki zwinięte; obrazy jako leniwe miniatury |
| IPC rdzeń → UI | zdarzenia grupowane (batch co klatkę), bez przesyłania dużych danych (pliki/obrazy przez ścieżki i protokół zasobów) |

### 14.8 Drobne funkcje — pełna lista (wszystko także z klawiatury i palety poleceń)
**Composer:** wiele linii (auto-wzrost do ~40% wysokości), `Enter` wyślij / `Shift+Enter` nowa linia (odwracalne w ustawieniach), **szkic zapisywany per sesja**, wklejanie obrazów i plików, przeciąganie, zrzut ekranu z composera, `@agentka` (adresowanie), `/komendy` (np. `/obsada`, `/model`, `/pamięć`, `/eksport`), wybór agentki i profilu modelu jako chipy, szacunek kosztu/tokenów przy dużych załącznikach, **sprawdzanie pisowni PL** (wbudowane w WebView2 + słownik użytkownika; do weryfikacji w F0), historia wysłanych (`Ctrl+↑/↓`), `↑` w pustym composerze = edytuj ostatnią wiadomość, „wklej jako zwykły tekst" (`Ctrl+Shift+V`), cofnięcie wysłania przez 3 s (opcjonalnie).

**Wiadomość (pasek akcji po najechaniu, fokusie lub prawym przyciskiem):** kopiuj (Markdown / tekst), **przeczytaj na głos** (głosem tej agentki), **ponów** (opcjonalnie innym modelem) — tworzy wariant obok (`‹ 1/3 ›`), **edytuj i wyślij ponownie** — tworzy odgałęzienie od tego miejsca, **kontynuuj** uciętą odpowiedź, **przekaż agentce…** *(dziennik zdarzeń jest append-only, a widoczna rozmowa to projekcja drzewa gałęzi — każda gałąź jest nieedytowaną historią, więc bloki myślenia dostawców pozostają ważne)*, rozgałęź do nowej sesji, cytuj/odpowiedz, **zapamiętaj** (do pamięci, z wyborem zakresu), ocena 👍/👎 z notatką, **szczegóły** (model, tokeny, koszt, opóźnienie, użyte narzędzia → skok do Osi czasu), ukryj z widoku (audyt zostaje), eksportuj. Znaczniki czasu względne z pełną datą w podpowiedzi.

**Wiadomość agentki:** awatar z pierścieniem koloru + imię + chip roli (np. „Delta · Wykonawczyni"); kroki narzędzi zwinięte do jednej linii statusu (ikona, opis, czas) z rozwinięciem; bloki kodu: kopiuj, zapisz jako plik, **uruchom w terminalu** (przez Broker), zawijanie; linki do plików otwierają podgląd; **karta „czeka na zatwierdzenie"** w wątku z przyciskiem przenoszącym do okna Brokera (samo zatwierdzenie tylko w oknie Brokera, §8.2).

**Sesje (panel lewy):** wyszukiwanie pełnotekstowe (`Ctrl+Shift+F`), projekty/foldery, tagi, przypięte, **kropka aktywności** przy sesjach pracujących w tle, znacznik nieprzeczytanych, sortowanie, zmiana nazwy (`F2`, automatyczny tytuł z pierwszych wiadomości), duplikuj jako szablon, archiwizuj, usuń z **cofnięciem przez 10 s**, eksport pojedynczej sesji do `.alfa`.

**Licznik kontekstu i kosztu:** pasek zużycia okna kontekstu sesji (z oznaczeniem kompaktowania), koszt sesji i dnia; ostrzeżenie przed przekroczeniem limitu (jeśli włączony).

**Zasobnik systemowy (tray):** ikona ze stanem (spoczynek / słucha / mówi / pracuje / błąd / mikrofon otwarty), menu: pokaż, nowa rozmowa, **Szybkie pytanie**, głos wł./wył., nie przeszkadzać, **STOP WSZYSTKIEGO**, wyjście. Zamknięcie okna = do zasobnika (konfigurowalne).

**Szybkie pytanie (Quick Ask):** globalny skrót (domyślnie `Ctrl+Alt+Space`, zmienialny, wykrywanie konfliktów, np. z PowerToys) → małe okno 640 px na środku ekranu z polem tekstowym; odpowiedź rozwija się pod spodem; `Enter` „otwórz w pełnym oknie"; `Esc` zamyka.

**Pigułka głosowa (mini-nakładka):** małe, przesuwane okno „zawsze na wierzchu" (ok. 220 × 48 px) z awatarem mówiącej agentki, falą głośności, stanem mikrofonu i przyciskami Stop/Wycisz; pokazuje się, gdy rozmawiasz głosem przy ukrytym oknie głównym. *(Każde dodatkowe okno Tauri kosztuje RAM — mierzone w F0; pigułka to minimalna strona bez frameworka.)*

**Powiadomienia:** natywne Windows (toast) gdy okno ukryte — zakończenie zadania, prośba o zatwierdzenie, błąd; w aplikacji — małe toasty w prawym dolnym rogu; tryb **nie przeszkadzać** (także automatycznie przy pełnym ekranie/grach).

**Dźwięki (earcony):** krótkie, ciche: start/stop słuchania, zadanie ukończone, błąd, prośba o zatwierdzenie; osobna głośność, wyłączalne.

**Podglądy plików:** obrazy, PDF, tekst/kod (z podświetleniem), audio/wideo, Markdown; dokumenty Office jako podgląd tekstowy (P1).

**Skróty (wszystkie zmienialne, z wykrywaniem konfliktów i regułą AltGr z §8.6):** `Ctrl+N` nowa rozmowa · `Ctrl+W` zamknij kartę · `Ctrl+Shift+T` przywróć zamkniętą · `Ctrl+Tab` / `Ctrl+1…9` karty · `Ctrl+P` szybkie przełączanie sesji · `Ctrl+B` panel sesji · `Ctrl+\` panel prawy · `Alt+1…6` panele (Agentki, Oś czasu, Pliki, Pamięć, Ekran, Głos) · `Ctrl+K` paleta · `Ctrl+F` szukaj w rozmowie · `Ctrl+Shift+F` szukaj wszędzie · `Ctrl+,` ustawienia · `Ctrl+/` ściągawka skrótów · `Ctrl+Shift+M` mikrofon wł./wył. · przytrzymanie `Spacji` (poza polem tekstowym) = mów · `Esc` — kolejno: zamknij menu/dialog → stop mowy → stop generowania · `F11` / `Ctrl+Shift+Enter` tryb skupienia · `↑` (pusty composer) edytuj ostatnią · `Ctrl+↑/↓` historia wysłanych · `F2` zmień nazwę · `Ctrl+=/-/0` zoom · `Ctrl+Alt+Space` szybkie pytanie · **`Ctrl+Shift+F12` STOP WSZYSTKIEGO**. Przeładowanie strony WebView (`F5`/`Ctrl+R`) jest wyłączone.

**Przewijanie i czytanie:** automatyczne przewijanie przy strumieniu wyłącza się, gdy przewiniesz w górę; przycisk „↓ nowe (3)"; pozycja zachowana przy zmianie wariantu/gałęzi; separatory dni; blok „myślenie" zwinięty z czasem trwania; tabele z przewijaniem poziomym; KaTeX/Mermaid ładowane dopiero przy użyciu; błąd na poziomie wiadomości z akcją „Ponów / inny model"; szkielety ładowania zamiast spinnerów dłuższych niż 300 ms.

**Cofanie jednym kliknięciem:** po każdej cofalnej akcji agentki (`fs.*`) toast „Cofnij" przez 8 s — skrót do dziennika cofania; karty kroków narzędzi mają przycisk „Cofnij" (np. „Delta: przeniesiono 14 plików · 2,1 s · Cofnij").

**Pasek zadań Windows:** postęp długiego zadania na ikonie i plakietka „czeka na zatwierdzenie". Zatwierdzenia **nigdy** w toaście — tylko w Broker-UI.

**Język i formaty:** poprawne liczby mnogie PL („1 plik, 2 pliki, 5 plików"), żeńskie formy czasowników agentek, daty/liczby/waluta w formacie pl-PL; przełącznik PL/EN per sesja; „Co nowego" po aktualizacji; eksport sesji do MD/HTML/PDF.

**Historia schowka w UI:** sekcja w palecie poleceń (wklej do composera), z wykluczeniem haseł i aplikacji z deny-listy oraz retencją.

**Dyktowanie do czatu:** podgląd transkryptu z możliwością poprawienia przed wysłaniem (opcjonalny tryb „sprawdź przed wysłaniem").

### 14.9 Tryb głosowy — szczegóły UI
- **Pełny tryb głosowy:** duży orb (Canvas 2D, reaguje na głośność, kolor mówiącej agentki; 30 kl./s, pauza w tle), pod nim **napisy na żywo**: Twoja wypowiedź najpierw szara (tekst częściowy), po zatwierdzeniu pełna; odpowiedź agentki z podświetleniem już wypowiedzianych słów i znacznikiem „przerwano tutaj".
- **Stany mikrofonu** (kolor + ikona + tekst, nigdy sam kolor): wyłączony · słucha · słyszy Cię · przetwarza · agentka mówi · wyciszony · nie przeszkadzać. Wskaźnik prywatności w zasobniku, gdy mikrofon jest otwarty.
- Przyciski: Stop mowy, Wycisz, Przełącz agentkę, Przejdź do tekstu; wybór urządzenia audio w jednym kliknięciu.

### 14.10 Ekrany do makiet (akceptujesz je przed implementacją UI — bramka ludzka)
1 Start/pusty stan (3 sugestie) · 2 Rozmowa (strumień, kroki narzędzi, karta zatwierdzenia, warianty odpowiedzi) · 3 Pełny tryb głosowy · 4 Pigułka głosowa · 5 Szybkie pytanie · 6 Panel Agentki / obsada ról · 7 Oś czasu i Replay · 8 Artefakty i podgląd pliku · 9 Inspektor pamięci · 10 Ekran (computer use) · 11 Ustawienia (drzewo + wyszukiwarka) · 12 Hub kont i kluczy + kreator · 13 Import / eksport `.alfa` · 14 Onboarding (każdy krok) · 15 Okno Brokera (zatwierdzenie, poziomy autonomii) · 16 Voice Lab i casting głosów · 17 Zdrowie systemu · 18 Stany błędów / offline / brak kluczy · 19 Kreator agentek · 20 Menu zasobnika. Każdy ekran w wariancie jasnym i ciemnym oraz w szerokości laptopa (1280 px efektywnie). Makiety powstają jako klikalne strony w Storybooku na prawdziwych komponentach (nie grafiki), więc od razu mierzą budżety §14.7.

---

## 15. Konfiguracja, kopie zapasowe, migracje

- Pliki `%APPDATA%\Alfa\config\*.toml` z **JSON Schema**; przeładowanie na żywo; profile (Prosty/Zaawansowany/Ekspert); historia w git z diffem i rollbackiem; **zmiany polityk Jądra tylko przez Broker (potwierdzenie w jego oknie, opcjonalnie Hello)**. Każdy moduł dostarcza własną stronę ustawień z manifestu.
- Każde ustawienie: opis, domyślna, zakres (globalny/sesja/agentka), reset, wyszukiwanie.
- **Drzewo:** Ogólne · Modele i dostawcy (konta, klucze, lokalne, mosty + karty zgodności) · Router i reguły · **Głos** (urządzenia, STT, TTS, tury i barge-in, słowa wywoławcze, weryfikacja mówcy, dyktowanie, czytanie, słownik wymowy, Voice Lab) · **Agentki** (Alfa/Beta/Gama/Delta, obsada ról, Kreator) · Uprawnienia i bezpieczeństwo · Komputer · Pamięć · Sesje i okna · Pliki · Logi i prywatność · Samonaprawa i ulepszanie · **Moduły** (lista, włącz/wyłącz, budżety) · **Urządzenia** (profil sprzętu per maszyna, tryb baterii) · **Import i eksport** (paczki `.alfa`, kopie zapasowe) · Wygląd · Skróty · Powiadomienia · Język · Aktualizacje · Zaawansowane (edytor surowy, flagi).
- **Konta i klucze** mają własną stronę w „Modele i dostawcy" (Hub kont, §5.6): dodawanie, test, limity, usuwanie — w dowolnym momencie.

### 15.1 Import / eksport (`transfer`) — przenoszenie między Twoimi maszynami, bez automatycznej synchronizacji
- **Eksport** do pojedynczego pliku **`.alfa`** (archiwum + manifest z wersją schematu i sumami kontrolnymi): wybierasz zakres — konfiguracja wspólna, agentki i biblie głosów, obsady ról, reguły, umiejętności/agenci z Kreatora, **wybrane sesje**, **wybrane zakresy pamięci**, artefakty (opcjonalnie), logi (domyślnie nie), nakładka maszyny (domyślnie nie). **Klucze API i sekrety nie wchodzą do eksportu** (decyzja 2026-10-04, CX-a: AGENTS.md wygrywa — bez osobnej opcji eksportu sekretów; klucze dodaje się na nowej maszynie w Ustawieniach → Konta). Opcjonalne szyfrowanie całej paczki hasłem.
- **Import:** podgląd zawartości i różnic (dry-run) → tryb *dodaj / scal / zastąp*, rozwiązywanie kolizji (id sesji, wpisy pamięci), migracja schematu (upcastery), **automatyczny snapshot przed importem** i jednoklikowy rollback.
- **Kopie zapasowe = zaplanowany eksport** (ten sam format i kod) do wskazanego katalogu, z rotacją; **test przywracania w CI**. Wersjonowanie IR i schematu zdarzeń, testy migracji.
- Eksport/import dostępny z UI, palety poleceń i głosem („Beta, wyeksportuj sesję X").

---

## 16. Zakres v1 i roadmapa

### 16.1 Priorytety
- **P0 (MVP):** jądro + moduły bazowe, **`accounts-hub` (dodawanie kluczy i kont później)**, czat z dostawcami A+B, **głos kaskadowy z barge-in i czterema głosami (v0 lokalnie bez kluczy, docelowe po castingu; profil A działa na baseline)**, obsada ról, **`transfer` P0-lite (konfiguracja + sesje)**, UI wg §14 (tokeny, budżety §14.7, zasobnik, Szybkie pytanie), Broker + audyt + cofanie + kill-switch, fs/shell/schowek, sesje z osobną pamięcią, logi v1, onboarding, **`device-profile`**.
- **P1:** **`transfer` pełny (pamięć, umiejętności, kopie zapasowe)**, pigułka głosowa, mosty CLI, GUI (UIA/wizja/OCR), przeglądarka, agentki + Scheduler + Marszałek, pełna pamięć, **głos rozszerzony** (słowa wywoławcze — opcjonalnie, weryfikacja mówcy, dyktowanie, czytanie, tryb S2S w chmurze), Ulepszacz R0–R2, helper/UAC, Office (Word/Excel).
- **P2 / poza v1:** R3, transkrypcja spotkań, A2A/debata, rejestrator makr/RPA, ARM64, tryb „Rada", wirtualny monitor.

### 16.2 Fale (harmonogram wyznaczają Twoje zatwierdzenia i testy na sprzęcie, nie pisanie kodu; rozmiary względne)
| Fala | Moduły / zakres | Kryteria akceptacji (mierzalne) | Sprzęt / człowiek |
|---|---|---|---|
| **0. Fundament i spike'i** | repo, `AGENTS.md`, CI, hooki, kontrakty jądra (`core-*`) + `SystemPort`-contract z fake'iem, `docs/vendor`, rdzeń `evals` (bez holdoutu), `voice-lab` jako narzędzie pomiarowe, ADR-y (lista w §20), `ui-kit` v0 (tokeny + podstawowe komponenty + Storybook); **spike'i time-boxed (≤ 1 tydz. każdy):** (a) pętla głosowa + AEC (własna referencja vs loopback vs Communications), (b) most CLI (`--permission-prompt-tool`, approvals, zimny start — wymaga tylko Twojego logowania do CLI, które masz), (e) **Voice Lab PL: tabela kandydatów × PL × licencja × streaming × zasoby × TTFB, pomiar na desktopie i laptopie**, (f) **pomiar RAM drzewa Tauri/WebView2 (1 i 3 okna)**, (h) **pomiary na klasach sprzętu** (§3.5): whisper.cpp **Vulkan na RX 9070 XT** i **CUDA na RTX 4050**, te same pod limitami baseline (6 rdzeni, 16 GB, VRAM 8 GB, korekta GPU ×2,2), RTF Pocket-PL przy 6 rdzeniach, tok/s modeli 4B/8B w llama.cpp, model KWS „Hej …", (i) **dane:** SQLCipher + sqlite-vec + FTS5 w jednej bazie, (j) **powłoka Windows:** Snap Layouts, Mica, pisownia PL w WebView2, toasty z AUMID przez launcher, (k) **Broker-UI** na wyższym poziomie integralności uruchamiany z usługi + test odrzucenia SendInput, (l, opcjonalny) ROCm/PyTorch na RDNA4. *(g) append-only vs bloki myślenia Anthropic — **odroczony do klucza Anthropic**, ADR (6) tymczasowy. Spike'i (c) UIA i (d) współdzielenie wejścia — przeniesione na początek F6.* | go/no-go: (a) profil A: p50 ≤ 2000 ms, p95 ≤ 3000 ms na desktopie z emulacją (+ korekta GPU) i na laptopie; (b) 100% próśb o uprawnienia trafia do naszego kanału, 0 odczytów tokenów, zimny start zmierzony; (e) WER PL ≤ 12% na korpusie własnym; Twoja ślepa ocena TTS 1–5 na 20 zdaniach PL, średnia ≥ 4,0 — no-go = zostają głosy v0 (nie blokuje F1); (f) budżety §3.4 i §14.7 ustalone; (h) 0 crashy w 1 h ciągłej pracy, VRAM w budżecie; (i)(k) działa / ADR; (UI) makiety 1–3 (§14.10) zaakceptowane przez Ciebie — pozostałe zatwierdzasz przed falą, która je implementuje | Tak: desktop + laptop, korpus własny (bramka #3), akceptacja makiet, przygotowanie maszyn (bramki #8–10) |
| **1. Rdzeń czatu** | `platform-windows` v1 (fs, procesy, schowek, okna, zasobnik, **skróty globalne + hook klawiatury dla PTT**; bez UIA/SendInput), **launcher i układ katalogów `%LOCALAPPDATA%\Alfa`** (aktualizacje i rollback w F3), `sessions`, `search`, `artifacts`, `device-profile`, **`accounts-hub`** (kreator kluczy/kont, katalog dostawców), **`transfer` v1 (P0-lite: konfiguracja + sesje `.alfa`)**, `providers-api` (Anthropic, OpenAI, **adapter generyczny**), **`providers-local` (llama.cpp Vulkan/CUDA) — wymagany w MVP: bez klucza jedyny „mózg"; model 3–4,5B Q4 pobierany w onboardingu**, `router` v1, `cost-meter`, `compliance` v0 (rejestr, tagi z katalogu, wyłącznik trasy), `memory` v0 (`remember/recall` per sesja), `ui-shell` + `ui-kit` (Svelte 5, tokeny §14.3) + **`ui-quick` (Szybkie pytanie, menu zasobnika)** + **Ustawienia** (drzewo + strony z manifestów) + **Oś czasu v0** (lista zdarzeń sesji), `shell-integration` + `notify` (zasobnik, toasty), logi v1 (Audyt tymczasowo pisze `core-log` z oznaczeniem `pre-broker`), minimalny onboarding. *Bez narzędzi agentek — sam czat; narzędzia dochodzą z Brokerem w F3* | **≥ 3 adaptery zielone na fixture'ach syntetycznych ze schematów API** (nagrywane automatycznie przy pierwszym kluczu; walidacja na żywo = nightly, nieblokująca) + lokalny llama.cpp na żywo; fallback: sztuczny 5xx/timeout → przełączenie ≤ 2 s bez utraty wiadomości; ≥ 3 sesje równolegle, 0 przecieków między sesjami (testy szpiegowskie); przyrost Private WS ≤ 5% po 1 h / 500 wiadomościach; axe: 0 naruszeń critical/serious; budżety lekkości i UI (§14.7) na desktopie z emulacją baseline; round-trip `.alfa` (config + sesje) desktop ↔ laptop; dodanie klucza (atrapa dostawcy) bez restartu | — |
| **2. Głos rdzeniowy + agentki** | `voice-audio/dsp/vad/turn/stt/tts/dialog/persona/cmd`, **`voice-wake` v0 (PTT/przełącznik, adresowanie po imieniu)**, `model-residency`, **`scheduler-lite`** (wyłączność głośnika/mikrofonu, kolejka mowy; *delegacja v0 = przekazanie tury/rozmowy innej personie, bez pracy w tle*), `personas` (Alfa/Beta/Gama/Delta) + obsada ról (*w F2 rola = prompt + polityka modelu; tokeny zdolności per rola dochodzą z Brokerem w F3*), **głosy v0: wbudowane głosy Pocket-PL/Piper o sprawdzonej licencji, ≥ 2 różne bazowe mówczynie + modyfikacja wysokości i tempa — bez kluczy**; **casting właściwy** po dodaniu klucza do usługi voice design (ElevenLabs/MiniMax); panele **Głos** i **Agentki** | zestaw testowy z korpusu (bramka #3), **podział dev/test, test zamrożony (hash)**: ≥ 300 wypowiedzi PL + mieszane PL/EN, warunki cisza/szum/głośniki/słuchawki; p50/p95 profilu A wg §6.4 (B/C po kluczach, nieblokujące); WER PL ≤ 12%; recall „stop/anuluj" ≥ 99% na ≥ 200 próbach, reakcja < 300 ms; precision backchannelu ≥ 95%; fałszywe przerwania ≤ 1/godz. przy 1 h odtwarzania TTS przez głośniki laptopa + tło TV bez Twojej mowy; prefiks ±1 słowo ≥ 90%; klasyfikacja intencji przerwań ≥ 90% na klasę (≥ 50 przykładów na klasę); odrębność głosów: cos-sim ECAPA między parami ≤ 0,6 + Twoja identyfikacja ABX ≥ 90%; zmiana obsady w locie bez restartu sesji | Tak: odsłuchy, sprzęt |
| **3. Safety Kernel + system (MVP)** | `safety-broker` + `broker-ui`, `watchdog`, `updater` (aktualizacje, rollback), audyt (Broker przejmuje strumień z nowym łańcuchem hashy), `undo-journal`, `risk-classifier`, kanał zatwierdzeń (okno Brokera, opcjonalnie Hello), poziomy autonomii L0–L4 z przełącznikiem w Ustawieniach, tokeny zdolności, **`agent-runtime` v0** (pętla narzędzi jednej agentki: plan→akcja→obserwacja→weryfikacja, budżety, checkpointy, anulowanie; bez równoległości i DAG), `tools-fs/shell/clipboard`, deny-listy; UI: Replay krok po kroku, toasty i karty „Cofnij", „uruchom w terminalu", karta „czeka na zatwierdzenie" | kill-switch: od klawisza do ciszy audio i zabicia wszystkich Job Objects **< 200 ms p95 z 50 prób pod obciążeniem UI**; cofalność: ≥ 200 losowych operacji `fs.*` (property-based) 100% + snapshot zakresu dla shella; „agentka zmienia Jądro / zatwierdza sama siebie" — ≥ 100 scenariuszy (w tym SendInput do Broker-UI) = 0 sukcesów; red-team injection (tekst/plik/strona/audio) ≥ 100 przypadków: 0 eskalacji i 0 egressu bez potwierdzenia; **eval narzędzi fs/shell na lokalnym modelu ≥ próg ustalony w F0**; **scenariusz MVP bez kluczy na desktopie z emulacją:** onboarding → rozmowa głosowa z barge-in → zadanie fs + Cofnij → kill-switch → eksport `.alfa` → import na laptopie | UAC, ewent. Hello |
| **4. Mosty i MCP** *(∥ F5)* | `agent-backends` (Claude Code, Codex; potem Grok Build, Kimi Code, `agy` eksperymentalnie — §5.5), „opaque worker", `compliance` v1 (karty zgodności w UI, archiwum regulaminów), `ui-terminal` (+ krok „mosty CLI" w onboardingu), `mcp` (klient + **serwer v0: schowek, okna**; UIA/zrzuty/rejestr dochodzą w F6; adapter generyczny API jest już w F1) | delegacja: postęp ≤ 1 s opóźnienia, anulowanie ≤ 2 s, 20/20 próśb o uprawnienia trafia do Broker-UI; monitor dostępu do plików (ETW) procesów Alfy: 0 odczytów `~/.claude`, `~/.codex`; wyłącznik: 0 wywołań wyłączonej trasy w 100 próbach | logowanie do CLI |
| **5. Agentki i orkiestracja + głos rozszerzony** *(∥ F4)* | `agent-runtime` v1 (wiele agentek równolegle, steering), `scheduler` (pełny, DAG), `marshal`, `agent-builder`, `triggers`, umiejętności; `voice-wake` v1 (słowa wywoławcze), `voice-speaker`, `voice-s2s`, **pigułka głosowa** (`ui-quick`), `platform-windows` v1.5 (SendInput tekstu, UIA `TextPattern` tylko do odczytu) → `voice-dictation`, `voice-readaloud` | agentki równolegle z blokadą ekranu/głośnika; steering uwzględniony w ≤ 1 kroku atomowym (20/20); 0 zakleszczeń w 1000 losowych scenariuszy schedulera; „most nie startuje z wyzwalacza" = 0/100; słowa wywoławcze: FAR ≤ 1/dzień na ≥ 24 h nagrań tła PL (TV, podcasty), FRR ≤ 5% na ≥ 200 Twoich pozytywach; weryfikacja właściciela: EER ≤ 3% (obce głosy: Common Voice PL + TTS; dla FAR ≤ 0,1% ≥ 3000 prób obcych) | korpus, testy akustyczne |
| **6. Computer use** | spike'i (c) UIA/SendInput/zrzuty + macierz aplikacji i (d) współdzielenie wejścia (≤ 1 tydz. każdy) → `platform-windows` v2 (UIA, SendInput, zrzuty), `tools-uia/vision/input/window`, `tools-system/net/media`, `tools-browser`, `tools-office` (Word/Excel), helper/UAC, panel Ekran, serwer MCP v1 (UIA, zrzuty, rejestr) | **wymaga klucza API lub mostu jako „mózgu"** (lokalny 3–4,5B nie osiągnie progu); własny zestaw ≥ 50 zadań w ≥ 5 kategoriach w VM: ≥ 85%; benchmark zewnętrzny (typu OSWorld / Windows Agent Arena — dostępność do weryfikacji): nie gorzej niż opublikowany wynik tego samego modelu − 5 pp | VM (Hyper-V: Win11 Pro — edycja do potwierdzenia) |
| **7. Pamięć pełna + transfer pełny** | 4 warstwy, konsolidacja, Inspektor, `forget` kaskadowo; `transfer` pełny (pamięć, umiejętności, kopie zapasowe z harmonogramem) | 0 przecieków w testach szpiegowskich; recall@5 ≥ 0,85 na ≥ 200 zapytaniach PL, zestaw zamrożony; kaskada `forget` zweryfikowana; round-trip `.alfa` desktop ↔ laptop bez utraty danych | — |
| **8. Samonaprawa i ulepszanie** | `diagnostician`, `improver`, `evals` (holdout), `plugin-runtime` (Wasm), „Zdrowie systemu" | **katalog awarii chaosowych** (≥ 20) naprawiony i cofalny; Ulepszacz nie zmienia Jądra ani progów (test) | — |
| **9. Dopieszczenie** | audyt a11y, wydajność (idle, bateria, wykrywanie gier/pełnego ekranu), kopie zapasowe+restore, „Co nowego", dokumentacja, pentest | pentest wykonuje model inny niż autor wg listy z THREAT_MODEL/OWASP: 0 ustaleń CVSS ≥ 9 otwartych; budżety lekkości utrzymane | — |

*Reguła: funkcja z §14.8 pojawia się w fali modułu, od którego zależy. Równoległość: F4 ∥ F5; w F0–F1 prace UI ∥ spike'i głosu.*

---

## 17. Ryzyka i mitygacje

| Ryzyko | Mitygacja |
|---|---|
| Zmiany regulaminów tras abonamentowych | Adaptery izolowane, podpisany rejestr z wyłącznikiem, egzekucja techniczna (§1.3), fallback lokalne/API |
| Most CLI omija zabezpieczenia | „Opaque worker", zatwierdzenia w naszym UI, worktree, wąski MCP |
| Agentka zatwierdza własne prośby / XSS→RCE | Broker-UI na wyższym poziomie integralności (potwierdzenie fizycznym wejściem) + zakaz gui.control wobec Alfy + markdown sanitizowany w Rust + ścisłe CSP; iframe bez IPC dla artefaktów HTML |
| **Skróty kolidujące z polskim AltGr** | Reguła zakazanych kombinacji `Ctrl+Alt+litera` + test CI; kill-switch `Ctrl+Shift+F12` |
| **Wersje side-by-side psują integrację z Windows** | Stały launcher i stały folder danych WebView2 (§1.2) |
| Jądro w procesie agentów | Broker na osobnym koncie, audyt append-only, kotwica hashy |
| **Wiedza modeli AI po cutoffie (WASI 0.3, windows-rs, Tauri 3, specta)** | Przypięte wersje, `docs/vendor`, docs.rs jako kontekst, kompilator/CI jako filtr halucynacji |
| **Błędna składnia Svelte 4/5** | Lint + hooki + Svelte MCP; plan B React |
| **AI nie słyszy jakości głosu** | Automaty jako proxy + bramka ludzka (odsłuch, korpus własny) |
| **Środowisko dev bez Windows/audio** | Fake'i, `windows-latest`, testy sprzętowe na Twoich maszynach |
| **Zróżnicowany sprzęt (desktop AMD RDNA4 16 GB, laptop NVIDIA 6 GB, baseline AMD RDNA3 8 GB niedostępny fizycznie)** | `device-profile` + profile A–D, nakładki per maszyna, emulacja baseline (limity), ścieżki Vulkan (desktop) i CUDA (laptop) w CI sprzętowym, tryb baterii i termika dla laptopa, fallback CPU; luka RDNA3 opisana w §3.5 |
| **Brak kluczy/kont na starcie** | `accounts-hub` (§5.6): program działa lokalnie (profil A, lokalny LLM jako „mózg", jakość rozmowy ograniczona), klucze dodawane później bez restartu; testy chmurowe na fixture'ach syntetycznych |
| Polski w STT/TTS słabszy niż zakładano | Voice Lab w F0, profil chmurowy, korpus własny; wyniki „nowszy = lepszy" nie zakładane |
| Lokalny TTS PL: akcent/jakość (Chatterbox zgłoszenia), niepotwierdzone modele | Kilku kandydatów, fallback chmurowy, ślepe A/B |
| Opóźnienie/echo, nierealne cele | Budżet per etap, spike (a), profile, p95 |
| Historia edytowana przy barge-in vs bloki myślenia | Historia append-only (§6.5), spike (g) |
| Wake-word „Alfa/Beta/Gama/Delta" — krótkie, fałszywe alarmy | Frazy 3–4-sylabowe „Hej …", bramka właściciela, korpus własny |
| Zawodność computer use | Trasy, weryfikacja, macierz app×trasa, benchmark zewnętrzny |
| Prompt injection (także dźwiękiem), zatrute MCP | Sesja `tainted`, trifecta, hash opisów MCP, adresat głosu, red-team z progiem |
| Samo-modyfikacja / Goodhart | Bez R3 w v1, Jądro z bramką, holdout, tylko zawężające auto |
| Rozrost zakresu / lekkość | Priorytety P0–P2, budżety w CI, moduły ładowane na żądanie |
| Łańcuch dostaw | Formaty bez kodu, hashe, cargo-deny/vet, minisign |
| Prywatność audio/ekran/logi | Lokalność, retencja 7 dni, crypto-shredding, deny-listy, „co poszło do chmury" |
| Koszty | Limit PLN (wyłączalny), alerty, budżet tła lokalny, szacunek przed zadaniem |
| **UI ciężkie/wolne na WebView2** | Budżety wydajności UI w CI (§14.7), natywna Mica zamiast CSS blur, fonty systemowe, wirtualizacja list, lazy ładowanie paneli i podświetlania kodu |

---

## 18. Weryfikacja

**Planu:** przegląd §1.3 z aktualnymi regulaminami (pobrać i zarchiwizować); ADR-y; `THREAT_MODEL.md`; zatwierdzenie zakresu MVP.

**Infrastruktura testowa:** `windows-latest` (build/testy bez sprzętu) + dwie Twoje maszyny jako self-hosted runnery (audio, wirtualny kabel audio, VM ze snapshotem): **desktop (RX 9070 XT, Vulkan/AMD)** i **laptop (RTX 4050, CUDA, bateria)**; **macierz testów: baseline (emulowany) × desktop-AMD × laptop-CUDA (zasilanie i bateria)**; test round-trip **import/eksport `.alfa` między maszynami**. Ewaluacje LLM niedeterministyczne → N ≥ 5, przedziały ufności, przypięte wersje modeli, budżet kosztów. Kontrakty mostów na **nagranych fixture'ach** + ręczny nightly.

**Rodzaje testów:** kontraktowe (`-contract` vs `-impl` vs `-fake`), `cargo test`, graf zależności modułów; UI: E2E (Playwright/CDP lub `tauri-driver`), Storybook, regresja wizualna, axe, klawiatura; **głos:** Voice Lab na korpusie własnym — p50/p95, WER, fałszywe przerwania, prefiks, klasyfikacja przerwań (kryteria per klasa, nie średnia), FAR/FRR słów wywoławczych, EER weryfikacji mówcy, odrębność głosów; **computer use:** zadania w VM + benchmark zewnętrzny, macierz app×trasa; **bezpieczeństwo:** red-team injection (także dźwiękowy) z progiem, egzekucja tokenów, testy „zmiana Jądra"/„sama-zatwierdza", brak dostępu do tokenów CLI, „most nie startuje z crona"; **samonaprawa:** katalog chaosowy ≥ 20; **izolacja pamięci:** testy szpiegowskie; **migracje/aktualizacje/kopie:** upcastery, update+rollback, restore; **lekkość:** budżety §3.4 jako testy (start, idle RAM/CPU, zwalnianie modułów).

---

## 19. Decyzje

**Zamknięte (Twoje odpowiedzi):**
- **Sprzęt:** podany PC (Ryzen 5 5600 · RX 7600 8 GB · 16 GB · Win11) to **minimum (baseline), na którym program ma działać normalnie**. Twoje maszyny: **desktop** Ryzen 7 5700X3D · RX 9070 XT 16 GB · 32 GB; **laptop** i7-13700H · RTX 4050 6 GB · 16 GB (§3.5, profile A–D).
- **Klucze i konta:** dodasz później → system dodawania w dowolnym momencie (`accounts-hub`, §5.6); program działa bez nich.
- **Przenoszenie między maszynami:** wyłącznie **import/eksport do pliku `.alfa`** (§15.1), bez automatycznej synchronizacji.
- **Agentki:** cztery — Alfa, Beta, Gama, Delta; program i pierwsza agentka noszą imię „Alfa"; role zmienne (obsada ról); głosy młodych dorosłych 18–25.
- **Priorytet i praca:** głos przed computer use; AI (Opus 5.5 / GPT-6 Sol) pracuje lokalnie na Twoim Windows + w chmurze; **sesje deweloperskie uruchamiasz Ty, na swoich zasadach uprawnień** (AI nie dostaje domyślnie dostępu spoza katalogu repo).
- **Modele:** API do zaawansowanej rozmowy; mosty: Claude Code i Codex na start, potem **Grok (xAI)** Build → Kimi Code → `agy` (eksperymentalnie); Qwen/GLM tylko przez API; „Grill" = Grok — potwierdzone.
- **Koszty:** limit miesięczny na API w PLN **ustawiany w aplikacji, z możliwością wyłączenia**.
- **Autonomia:** wysoka/maksymalna, zmienna w Ustawieniach (start L3, L4 „Maks").
- **Jakość:** automerge po zielonym CI + przeglądzie drugiego modelu; Jądro/Broker zawsze przez Ciebie; jakość wg Definition of Done (§4.4).

**Do uzupełnienia później (nie blokuje planu — działam na wartościach domyślnych):**
1. **Klucze/konta** (ElevenLabs, Cartesia, OpenAI, Google, xAI, MiniMax…) — dodasz przez `accounts-hub`, gdy je zdobędziesz; do tego czasu F0 i MVP idą na profilu A i atrapach.
2. **Czy któraś z Twoich maszyn ma lub może pożyczyć kartę AMD RDNA3 (RX 7600)** — tylko dla fizycznego testu baseline'u; brak = emulacja i ryzyko opisane w §3.5.
3. **Edycja Windows 11** na desktopie (Home/Pro) — Hyper-V/Windows Sandbox do testów computer use (F6) wymagają Pro; przy Home zostaje VirtualBox/VMware.
4. **Ewentualne inne maszyny** — dodają się przez `device-profile` (autodetekcja); nie wymagają zmian w planie.

---

## 20. Po zatwierdzeniu — co zrobię w repo

Na gałęzi `ccr-af4b63c6-3fyzaj`:
1. `docs/PLAN.md` (ten plan), `docs/ARCHITECTURE.md`, `docs/AI_WORKFLOW.md`, `docs/VOICE.md` (Voice Suite), `docs/PERSONAS.md` (biblie głosu Alfa/Beta/Gama/Delta, obsady ról), `docs/UI.md` (kierunek wizualny, tokeny, układ, budżety, drobne funkcje, lista makiet — §14), `docs/THREAT_MODEL.md`.
2. `docs/ADR/0001…` — (1) stos Rust + Tauri 2 + Svelte 5, (2) mikrojądro i format modułu (trójka crate'ów, manifest), (3) izolacja Brokera + Broker-UI na wyższym poziomie integralności, (4) ML runtime bez Pythona (whisper.cpp Vulkan/CUDA, ONNX CPU), (5) `AgentBackend` vs `ModelProvider`, (6) historia append-only + gałęzie, (7) instalacja: launcher + wersje side-by-side + stały folder WebView2, (8) dane: SQLite szyfrowane + sqlite-vec + FTS5, (9) markdown renderowany w Rust, iframe tylko dla artefaktów, (10) reguła skrótów globalnych (AltGr), (11) silniki głosu v0 i audio (`wasapi`, AEC z własną referencją), (12) wtyczki wasmtime + WIT, (13) kontrakty i codegen TS (tauri-specta/ts-rs), (14) lokalny LLM llama.cpp (Vulkan/CUDA) zamiast Ollamy, (15) model uprawnień: tokeny zdolności i poziomy L0–L4. Każda decyzja z §1.2 ma swój ADR.
2a. `docs/ACCEPTANCE.md` — zestawy i progi akceptacyjne fal (z §16.2), zamrażane hashami w `evals/`.
3. `docs/modules/<moduł>/SPEC.md` — szkielety SPEC dla modułów P0 (`core-*`, `platform-windows`, `sessions`, `search`, `artifacts`, `memory`, `accounts-hub`, `providers-api`, `providers-local`, `router`, `cost-meter`, `compliance`, `transfer`, `device-profile`, `ui-shell`, `ui-kit`, `ui-quick`, `shell-integration`, `notify`, `voice-audio…cmd`, `voice-wake`, `model-residency`, `scheduler-lite`, `personas`, `agent-runtime`, `safety-broker`, `broker-ui`, `watchdog`, `updater`, `undo-journal`, `risk-classifier`, `tools-fs/shell/clipboard`) oraz `docs/formats/alfa-package.md` (format paczki `.alfa`, `transfer`).
4. `docs/compliance/subscription-routes.md`, `docs/compliance/compliance-registry.json` (szkic ze źródłami), `docs/compliance/archive/` (kopie regulaminów), szkic `providers-catalog/*.toml` (§5.6).
5. `AGENTS.md` (≤ ~100 linii) + `CLAUDE.md` (`@AGENTS.md`), `README.md` (PL).
6. Commit, push, **draft PR**, subskrypcja zdarzeń PR.

---

## Źródła (weryfikacja z 29.09.2026)
- Claude Code — Legal and compliance: https://code.claude.com/docs/en/legal-and-compliance
- Claude Help — Agent SDK z planem Claude: https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan
- OpenAI Codex — „Sign in with ChatGPT" w forkach: https://github.com/openai/codex/discussions/8338
- Gemini CLI — ToS: https://geminicli.com/docs/resources/tos-privacy/ · https://github.com/google-gemini/gemini-cli/discussions/22970
- Smart Turn (PL): https://github.com/pipecat-ai/smart-turn · Pocket TTS: https://github.com/kyutai-labs/pocket-tts · Chatterbox: https://github.com/resemble-ai/chatterbox · whisper.cpp: https://github.com/ggml-org/whisper.cpp · wasapi-rs: https://github.com/HEnquist/wasapi-rs · uiautomation-rs: https://github.com/leexgone/uiautomation-rs · windows-rs #4867: https://github.com/microsoft/windows-rs/issues/4867 · Svelte AI tools: https://github.com/sveltejs/ai-tools
- **Uwaga o wiarygodności:** oceny jakości i obsługi języka polskiego konkretnych modeli głosowych (TTS/STT), a także niektóre dane o zasobach (RAM Tauri, WER) pochodzą z wyników wyszukiwania i źródeł wtórnych (część domen była zablokowana) i są **hipotezami do zmierzenia w F0**, nie ustaleniami.
