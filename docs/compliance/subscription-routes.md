# Trasy abonamentowe (mosty CLI) — zasady zgodności

Źródło prawdy: `docs/PLAN.md` §1.3, §5.2, §5.5, §5.6, §8.5. Data weryfikacji regulaminów: **29.09.2026**.
Ten dokument opisuje zasady; stan maszynowy trzyma `compliance-registry.json` (moduł `compliance`), kopie regulaminów — `archive/`.

Oznaczenia pewności:
- **[V]** — potwierdzone na stronie źródłowej (URL w sekcji „Źródła" planu).
- **[W]** — źródło wtórne (cytat z forum/artykułu; strona dostawcy była niedostępna dla badania).
- **[?]** — brak jednoznacznego zapisu; interpretacja do wyjaśnienia.

Statusy trasy: **zielony** (wolno, po weryfikacji), **szary — do weryfikacji** (wyłączona do czasu weryfikacji), **zabronione** (nie budujemy; dany model tylko przez API).

## 1. Siedem zasad (§1.3 planu)

1. **Most abonamentowy = sterowanie oficjalnym, niezmodyfikowanym CLI** (`claude`, `codex`, potem `grok`, `kimi`, `agy`), do którego **loguje się użytkownik** (w wbudowanym terminalu ConPTY, §5.6), przez jego oficjalne tryby nieinteraktywne/SDK/app-server. **Kod Alfy nigdy nie czyta ani nie przechowuje tokenów** tych narzędzi.
2. Most = kontrakt **`AgentBackend`** (zadanie → strumień zdarzeń), „opaque worker" (§8.5), **nie** surowe `chat.completions` na koncie abonamentowym.
3. **Egzekucja techniczna** (szczegóły w §2 niżej): deny-lista domen/aplikacji dostawców w GUI/przeglądarce; deny-lista ścieżek poświadczeń w `fs.*`/shellu; przypięte wersje CLI (nieznana wersja wyłącza trasę); osobne statusy „CLI `-p`" i „Agent SDK".
4. **Uruchamianie:** domyślnie na żądanie użytkownika; harmonogramy mostów tylko jawnie włączone przez użytkownika, z limitami; Ulepszacz (§12) mostów nie używa. Kryterium F5: „most nie startuje z wyzwalacza" = 0/100.
5. **Rejestr zgodności** (`compliance-registry.json`): status, data weryfikacji, cytat źródła + archiwalna kopia regulaminu, wyłącznik trasy, karta zgodności w UI (Ustawienia → Modele i dostawcy → mosty). **Nieświeży rejestr degraduje trasę do „szarej".**
6. **Wykluczone:** automatyzacja webowych UI dostawców, wyciąganie ciasteczek/tokenów, modyfikowanie binariów CLI, zmiana User-Agenta.
7. **Redukcja kosztów:** modele lokalne (głos, zadania tanie), mosty CLI do ciężkiej pracy, API tam, gdzie się opłaca — kolejność wyboru w Routerze (§5.4), nie obejście regulaminów.

## 2. Egzekucja techniczna

Wszystkie poniższe listy należą do **Jądra** (§8.1) — agentki ani Ulepszacz ich nie zmieniają.

### 2.1 Deny-lista domen i aplikacji dostawców
- Zakres: `tools-browser`, `tools-uia`, `tools-input`, `tools-window`, `gui.control(...)` — agent nie „używa" webowego UI ani aplikacji desktopowych dostawców planów (claude.ai, chatgpt.com, gemini.google.com, grok.com, kimi.com, aplikacje desktopowe Claude/ChatGPT itp.; pełna lista w rejestrze, pole `providers[].deny_domains` — do uzupełnienia w F4).
- Deny-lista obejmuje też zrzuty ekranu/OCR (§8.7): okna tych aplikacji są wykluczone ze zrzutów.
- Wyjątek: **wbudowany terminal ConPTY do logowania** — obsługuje go wyłącznie użytkownik; Alfa nie czyta jego bufora poza wykryciem stanu „zalogowano/nie".

### 2.2 Deny-lista ścieżek poświadczeń
Zakres: `fs.*`, `tools-shell` (analiza argumentów + snapshot), MCP-serwer Alfy. Zablokowane dla odczytu, zapisu, kopiowania i listowania:
- `%USERPROFILE%\.claude\` (`~/.claude`), `%USERPROFILE%\.codex\` (`~/.codex`); analogiczne katalogi kolejnych CLI (`~/.grok`, `~/.kimi`, `~/.agy` — nazwy do potwierdzenia przy włączaniu trasy [?]),
- profile przeglądarek (`%LOCALAPPDATA%\Google\Chrome\User Data`, `%LOCALAPPDATA%\Microsoft\Edge\User Data`, `%APPDATA%\Mozilla\Firefox\Profiles` itd.),
- Windows Credential Manager (API `Cred*`, `cmdkey`, `vaultcmd`) — dostęp tylko przez `accounts-hub` do **własnych** wpisów Alfy,
- zmienne środowiskowe z tokenami CLI (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CODEX_*` itp.) nie są przekazywane do procesów narzędziowych agentek.
- Kryterium F4: monitor ETW dostępu do plików procesów Alfy — **0 odczytów `~/.claude`, `~/.codex`**.

### 2.3 Przypięte wersje CLI
- Rejestr trzyma `cli_pinned_version` per trasa (na razie `null` — ustala spike (b) w F0 / F4).
- Przy starcie mostu: `claude --version` / `codex --version` itd.; wersja spoza listy przypiętych → trasa **wyłączona** z komunikatem w karcie zgodności (nowa wersja może zmienić tryby, flagi lub regulamin).
- Aktualizacja CLI = ręczna decyzja użytkownika + ponowna weryfikacja trasy (§4).
- Binaria CLI **nie są modyfikowane**; integralność sprawdzana hashem pliku wykonywalnego przy przypinaniu.

### 2.4 Uruchamianie na żądanie
- Most uruchamiany tylko z sesji użytkownika (tekst/głos/paleta) lub z harmonogramu **jawnie** oznaczonego `allow_bridges: true` z limitem wywołań/dzień.
- `triggers`, `improver`, nocna konsolidacja pamięci — mosty zabronione (test w F5/F8).
- Osobny status w rejestrze dla trybu **CLI `-p`** i **Agent SDK**; SDK włączane osobno.

## 3. „Opaque worker" (§8.5)

- Most pracuje w **worktree/kopii** katalogu roboczego sesji, nigdy bezpośrednio na profilu użytkownika.
- Natywne prośby CLI o uprawnienia („ask") są **przekierowane do Broker-UI**: Claude Code — `--permission-prompt-tool`; Codex — approvals przez app-server (**do potwierdzenia w spike (b)** [?]). Kryterium: 100% (F0) / 20 z 20 (F4) próśb trafia do naszego kanału.
- Przez **MCP-serwer Alfy** most dostaje tylko narzędzia specyficzne dla Windows (F4: schowek, okna; F6: UIA, zrzuty, rejestr), **bez fs/shell** (te ma natywnie w worktree).
- **Cofanie** = snapshot przed/po (undo-journal); pamięć tylko przez `recall`; brak dostępu do pamięci globalnej.
- Zdarzenia CLI trafiają do strumienia Audyt oznaczone **„niezależnie niezweryfikowane"** (§13).
- Zimny start mostu to sekundy → mosty **nie** są na ścieżce głosowej (§5.2).
- Limity okien planów wykrywane reaktywnie (429/„wyczerpane okno") + estymata; stan pokazywany w UI (§14.4).

## 4. Tabela tras (stan z 29.09.2026)

| Trasa | Status | Tryb | Co wolno | Czego nie wolno | Źródło + data | Pewność |
|---|---|---|---|---|---|---|
| **Claude Code** (plan Claude Pro/Max) | **zielony** (CLI); **szary** (Agent SDK) | CLI `claude -p` (stream-json, `--permission-prompt-tool`); SDK osobny status | Logowanie użytkownika własnym planem do **niezmodyfikowanego binarnego Claude Code**; sterowanie oficjalnym trybem nieinteraktywnym; zwykłe, indywidualne użycie w limitach Pro/Max | Oferowanie logowania Claude.ai przez oprogramowanie trzecie; kierowanie ruchu przez dane logowania planu; przechowywanie tokenów; użycie poza „zwykłe indywidualne" (harmonogramy masowe) | code.claude.com „Legal and compliance"; support.claude.com „Agent SDK z planem" — 29.09.2026. Zasady SDK w ruchu (kredyt SDK ogłoszony i wstrzymany 15.06.2026) | [V] CLI; SDK [V] stan „w ruchu" |
| **Codex CLI** (plan ChatGPT) | **szary — ostrożnie** (do decyzji po spike (b)) | CLI nieinteraktywne / app-server (approvals — [?]) | Codex CLI oficjalnie działa z planami ChatGPT; logowanie użytkownika w niezmodyfikowanym CLI | „Bring your own plan" dla aplikacji trzecich — **bez oficjalnej odpowiedzi**; nie przechowywać tokenów; nie forkować/modyfikować CLI | github.com/openai/codex/discussions/8338 — 29.09.2026 | [V] (brak odpowiedzi to też ustalenie) |
| **Grok Build** (xAI) | **szary** (subskrypcja) → **zielony** (klucz API) | `grok -p --output-format streaming-json`, wznawianie sesji, ACP; Apache-2.0 | Domyślnie **klucz API xAI** (adapter generyczny lub CLI z kluczem); najlepiej udokumentowany headless | Subskrypcja: regulamin nie mówi wprost o sterowaniu z własnej aplikacji; AUP zakazuje scrapingu/destylacji | strona xAI niedostępna dla badania; brak URL w planie — do pobrania i archiwizacji | [?] subskrypcja; [W] AUP |
| **Kimi Code CLI** (Moonshot) | **szary — do weryfikacji** (plan: „zielony/szary") | `kimi -p --output-format stream-json`, `kimi acp`; MIT | Osobisty użytek na własnym planie; jedna wspólna kwota | Modyfikowanie User-Agenta; użycie niezgodne z „osobistym" | cytat wtórny; brak URL w planie — do pobrania i archiwizacji | [W] |
| **`agy`** (Antigravity CLI, Gemini) | **szary — eksperymentalny** | `agy -p` (błąd: bez TTY pusty wynik) | Wg cytatu z forum uruchamianie oficjalnego `agy` jako procesu potomnego to „supported workflow" | ToS Gemini CLI: dostęp przez oprogramowanie trzecie = naruszenie; bany za przechwytywanie OAuth; osobiste logowanie konsumenckie do Gemini CLI wygaszone 18.06.2026 | geminicli.com ToS; github.com/google-gemini/gemini-cli/discussions/22970 — 29.09.2026 | ToS [V]; „supported workflow" [W]; zapisy **sprzeczne** → Gemini przez klucz API |
| **Qwen Coding / Token Plan** | **zabronione** | — | Modele Qwen **wyłącznie przez API** (DashScope, klucz przypisany do regionu) | Regulamin planu: „nie używać do skryptów i scenariuszy nieinteraktywnych" | cytat wtórny; brak URL w planie | [W] |
| **GLM / ZCode** (Z.ai) | **zabronione** | — | Modele GLM **wyłącznie przez API** (Z.ai, endpoint OpenAI + Anthropic) | Plan tylko w „oficjalnie wspieranych narzędziach"; zgłaszane bany | cytat wtórny; brak URL w planie | [W] |

Uwagi:
- „Zielony" nigdy nie oznacza „bez logowania": każda trasa wymaga, by użytkownik sam zalogował się do CLI (bramka ludzka #1).
- Trasy „szare" są **wyłączone** (`enabled_by_default: false`), widoczne w UI z kartą zgodności i przyciskiem „Zweryfikuj".
- Trasy „zabronione" nie mają implementacji mostu; modele są dostępne w klasie B (API) przez adapter generyczny.

## 5. Tagi prywatności i jurysdykcji (§5.5)

Wpisywane do rejestru (`privacy_tags`), egzekwowane przez Router: sesja z tagiem **„prywatne" nie kieruje ruchu** do tras CN ani „może trenować".

| Tag | Znaczenie | Dotyczy |
|---|---|---|
| `google-personal-may-train` | konto osobiste Google — dane mogą trenować modele | Gemini (konto konsumenckie), `agy` |
| `google-paid-eea-no-train` | klucz płatny / EOG — nie trenuje | Google API (klucz płatny) |
| `xai-retention-30d` | retencja 30 dni | xAI API, Grok Build przez klucz |
| `cn-may-train` | dane w Chinach/Singapurze, może trenować | DeepSeek, Kimi (Moonshot) — także Kimi Code CLI |
| `sg` | Singapur | Z.ai, Alibaba/DashScope (Singapur) |
| `eu` | UE (Frankfurt) | Alibaba/DashScope (Frankfurt) |
| `unknown` | plan nie potwierdza — do weryfikacji | Anthropic, OpenAI, MiniMax, BytePlus, OpenRouter, Mistral, ElevenLabs, Cartesia, Azure, Soniox, Deepgram |

Tagi `unknown` traktowane przez Router **jak „może trenować"** do czasu weryfikacji (zasada ostrożności).

## 6. Procedura weryfikacji trasy przed włączeniem

1. **Pobranie regulaminu** (ToS/AUP/strona planu, polityka prywatności) z oficjalnej domeny; zapis kopii w `archive/` (URL, data, hash — patrz `archive/README.md`). Źródło wtórne **nie wystarcza** do statusu „zielony".
2. **Cytat**: do rejestru trafia dosłowny fragment (pole `sources[].quote`) z URL i `retrieved_at`; pewność `[V]`.
3. **Ocena** wg zasad §1: (a) czy dozwolone jest logowanie użytkownika do oficjalnego CLI i sterowanie trybem nieinteraktywnym z własnej aplikacji, (b) zakazy (UA, tokeny, skrypty), (c) tag prywatności. Wynik: zielony / szary / zabronione + `allowed` / `forbidden`.
4. **Spike techniczny**: przypięcie wersji CLI (`cli_pinned_version` + hash), test przekierowania uprawnień do Broker-UI, test ETW (0 odczytów katalogów poświadczeń), zimny start.
5. **Wpis w rejestrze** z `verified_at`; podpis rejestru (klucz Jądra, §17 „podpisany rejestr"); trasa nadal `enabled_by_default: false` — **włącza ją użytkownik** w karcie zgodności.
6. Zmiany statusu i rejestru zapisywane w strumieniu Audyt.

## 7. Odświeżanie i degradacja

- `max_age_days` (rejestr; start: **30**). Jeśli `dziś − verified_at > max_age_days` → trasa **degraduje do „szary"** automatycznie (moduł `compliance`), niezależnie od poprzedniego statusu; UI pokazuje „wymaga ponownej weryfikacji".
- Ponowna weryfikacja = kroki 1–3 z §6 (spike techniczny tylko przy zmianie wersji CLI).
- Zmiana wersji CLI, zmiana regulaminu (inny hash pobranej strony) lub błąd 4xx/„unsupported client" z CLI → natychmiastowa degradacja do „szary".
- Trasy „zabronione" nie są odświeżane automatycznie; zmiana statusu wymaga ręcznej weryfikacji i ADR.

## 8. Co archiwizujemy w `archive/`

- Pełne kopie stron regulaminów (ToS, AUP, strona planu, polityka prywatności, wpisy pomocy/dyskusji użyte jako źródło) dla każdej trasy i każdego dostawcy z rejestru, w formacie opisanym w `archive/README.md` (HTML + tekst, URL, data pobrania, hash SHA-256).
- Każdą **wersję** — stara kopia zostaje po zmianie regulaminu (diff to dowód „co się zmieniło").
- Cytaty wtórne [W]: kopia strony wtórnej + adnotacja, że to nie źródło pierwotne.
- Nie archiwizujemy: treści rozmów, tokenów, danych logowania.

Na dziś archiwum jest **puste** (tylko README) — pobranie i archiwizacja to pierwsze zadanie `compliance` v0/v1 (F1/F4) oraz punkt „Weryfikacja planu" (§18).

## 9. Kryteria testowe egzekucji (z §16.2 planu)

| Fala | Test | Próg |
|---|---|---|
| F0 spike (b) | prośby CLI o uprawnienia trafiają do naszego kanału; odczyty tokenów; zimny start | 100% / 0 odczytów / zmierzony |
| F4 | delegacja: opóźnienie postępu, anulowanie, prośby do Broker-UI | ≤ 1 s / ≤ 2 s / 20 z 20 |
| F4 | monitor ETW dostępu do plików procesów Alfy: `~/.claude`, `~/.codex` | 0 odczytów |
| F4 | wyłącznik trasy: wywołania wyłączonej trasy | 0 w 100 próbach |
| F5 | „most nie startuje z wyzwalacza" (harmonogram/Ulepszacz) | 0 z 100 |
| F8 | Ulepszacz nie zmienia tagów prywatności, deny-list ani rejestru | test w Jądrze |
