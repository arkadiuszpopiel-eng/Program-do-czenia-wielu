# Fakty czynności, własne persony i Jarvis — 12.11.2026

**Kandydat do przeglądu, nie zakończony odbiór Enterprise.** Naprawiono brak danych
czynności w kontekście kolejnej tury oraz awarie UI dla własnych person. Jakość
odpowiedzi prawdziwego modelu w sprzecznej historii nadal nie została odebrana.
Pełne pokrycie UI: **961/961** po jednorazowym powtórzeniu całej drugiej części;
pierwotna sporadyczna porażka pozostaje niewyjaśniona. To nie był czysty pierwszy przebieg.

[PR24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24)
pozostaje draft, z headem `cd55cc865b436517507a0eb5b14bf38bdb135d85`.
Nowy kod jest na [osobnej gałęzi](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/tree/fix/enterprise-evidence-personas),
aby zachować oczekujące CI poprzednika. Nie wykonano merge ani niezależnego przeglądu.

## Co zmienił kod

### #58 — przekazanie zapisów wybranej gałęzi

`app-store` czyta w jednej transakcji metadane wcześniejszych tur, Replay i powiązanie
checkpointu. Projekcja rozróżnia zapisane wykonanie, oczekiwanie na zgodę, błąd/odmowę,
trwanie, Undo i brak weryfikacji. Nieznane dane pozostają nieznane; CLI, stare wpisy
Done z samą intencją i niespójne powiązania nie są promowane do potwierdzonej akcji.
Powtórzone identyfikatory są deduplikowane, zakres obejmuje tylko wybraną gałąź/sesję.

`app-chat` dodaje te dane jako osobną wiadomość User przed niezmienionym ostatnim
celem. Nie powstają fikcyjne ToolUse/ToolResult ani dodatkowa instrukcja System.
Historia pozostaje append-only, tekst odpowiedzi modelu nie jest przepisywany.
Świeża sesja bez poprzedniej tury agentki zachowuje poprzednie żądanie.
Nota ma najwyżej cztery szczegóły i 1536 bajtów; nie zawiera ścieżek, argumentów,
wyników, prywatnych etykiet ani surowych identyfikatorów. Liczniki obejmują cały
wybrany zakres, także gdy szczegóły są przycięte.

RecordedDone oznacza status utrwalonego zapisu, nie dowód obecnego skutku na komputerze,
zaliczonego testu lub przyczyny odmowy. Przekazanie faktów nie gwarantuje, że model
poprawnie je opisze. Dlatego #58 pozostaje otwarte do próby semantycznej.

### #22/#48/#68 — własne persony, role i długie nazwy

Odtworzono awarie `undefined.glyph` i `undefined.name`: Rust dopuszczał własny ID
persony, a SDK i UI zakładały cztery wpisy w słowniku. SDK używa teraz otwartych
identyfikatorów; wiadomości, zgody, kokpit, Atlas i wybór adresatki mają bezpieczny
fallback. Brak metadanych oznacza neutralny awatar i ID, bez wymyślania imienia.
To nie jest jeszcze pełne ładowanie własnych nazw/grafik z biblioteki.

Biblioteka Kreatora zwraca opcjonalne `role_id` z rzeczywistego manifestu obok nazwy
wyświetlanej. Można jawnie dodać/odłączyć jedną rolę w rozmowie, zachowując pozostałe.
Starszy DTO bez role_id pozostaje zgodny; UI nie zgaduje identyfikatora z nazwy.
Jawne @imię ma pierwszeństwo przed menu adresatki. Odłączona osoba nie zostaje
nieaktualnym wyborem. Etykiety do 64 znaków mieszczą się na 320 px; pełny tekst jest
dostępny, gdy wizualnie skrócono go w composerze.

Atlas z wcześniejszego przyrostu to opcjonalna mapa świata offline oraz rzeczywiste
stany aplikacji i skróty. Bez geolokalizacji, wymyślonych tras, fontów webowych,
nowych zależności lub WebGL. Nie zmieniono pięciu trybów zgód ani uprawnień.

## Testy przypisane do źródeł

Liczby poniżej nie są sumowane między nakładającymi się seriami.

| Źródło | Kontrola | Wynik i zakres |
|---|---|---|
| `c5021d7a` | pełny Rust w partiach | **3628 zaliczonych / 0 błędów / 27 pominiętych**, 71/71 etapów; workspace/all-features/locked |
| `c5021d7a` | kompletność Rust | 884/884 binariów testowych, 264/264 celów doctest, bez braków; 1147 podsumowań libtest i osobny no_alloc z harness=false, kod 0 |
| `c5021d7a` | Store | 21/0/0, w tym 19 nowych przypadków |
| `c5021d7a` | właściwa ścieżka Core | RED 0/1 → GREEN 3/0/0; realne operacje TEMP, model i porty sprzętowe jako atrapy |
| `c5021d7a` | Clippy / deny | Clippy workspace/all-targets 0 bez ostrzeżeń; cztery kontrole cargo-deny OK, zachowane nieblokujące ostrzeżenia bazowe |
| `c7664edc` | IPC/Kreator/Core | RED 4/3 → GREEN 15/0/0; zgodność starszego DTO, rola z biblioteki, własna persona i restart |
| `c7664edc` | Clippy | workspace/all-targets 0, bez ostrzeżeń |
| `c2c461e7` | rzeczywisty LlamaCodec | 1/0/0: 30 przypadków, 60 ciał native-tools/JSONSchema, pięć person × Standard/Jarvis × trzy stany; dostawca odpowiedzi jako atrapa |
| `c2c461e7` | Clippy / deny / lock | 0; bez ostrzeżeń Clippy; cztery kontrole deny OK; tylko nowa krawędź testowa do już obecnego lib-openai-compat, te same pakiety/wersje/checksumy |
| `32a430aa` | Vitest | **593/593**, 106 plików; późniejsze zmiany dotyczą Svelte/CSS, bez zmiany badanej logiki TS |
| `e95883f9` | UI ukierunkowane | **46/46**, Chromium/headless/fake IPC |
| `894798a7` | ostatnia poprawka układu | **1/1**, długie ID w szerokim Atlasie |
| `af687d78` | integracje po połączeniu | **16/0/0**: IPC5, computer5, kontekst3, własna persona2, wire1; porty modelu/sprzętu jako atrapy |
| `894798a7` | pełne UI | **961/961**, bez pominięć, flakies i wewnętrznych retry w końcowym pokryciu; pierwotne 345/1/615 zachowane |
| `894798a7` | typy/lint/build/bundle/format zmienionych plików | wszystkie kod 0, svelte-check 0 błędów/0 ostrzeżeń; główny JS około 136,3 KiB gzip przy limicie 150 KiB |

Produkcyjne źródła Rust c766 są identyczne w UI894. Późniejszy c2 dodaje wyłącznie
integrację testową i zależność deweloperską; nie jest powtórzeniem pełnego Rust na
innym commicie. Po połączeniu porównanie apps/desktop/ui i packages z 894 jest puste; pełny UI dotyczy identycznych wejść frontendu.

Rzeczywisty test pliku TEMP: odczyt → nadpisanie → następna tura → ponowna budowa
Core nad tą samą zaszyfrowaną bazą i sejfem testowym → Undo odtwarzające poprzednie
bajty → następna tura. Odmowa zapisu poza zakresem nie tworzy pliku. Osobna próba
Undo nowego pliku po restarcie na Linuksie **niezaliczona: Trash Windows-only zwrócił
unsupported**; nie zastąpiono jej deklaracją zaliczenia na podstawie cofnięcia nadpisania.

Kontrola formatu przewodu #48 sprawdza faktyczny Core → dopasowanie okna 8k →
produkcyjny LlamaCodec, ale **bez HTTP/serwera/modelu**. Zachowano 60 ciał żądań i hashe,
30 z response_format. Dane, tożsamość i cel przechodzą bez fikcyjnych akcji i podmiany
odpowiedzi. Nie jest to 60 odpowiedzi modelu ani pomiar rzeczywistego tokenizera/GPU.
Pierwszy test miał 0/1 na końcowym warunku pokrycia: fixture WorkdirNone nie uruchamiał
narzędzi. Zmieniono wyłącznie fixture na Default, zachowując warunek, następnie 1/1
i osobne 1/1 z eksportem. Oryginalny błąd zachowano.

## Niezaliczenia, powtórzenia i dług

Monolityczny `cargo test --locked --workspace --all-features --no-fail-fast -j2`
zakończył się kodem 101 **przed testami wykonawczymi**: pełny dysk 32 GB i linker Bus
error. Zapisano log, usunięto tylko własne stare artefakty testowe i uruchomiono cały
ten sam zakres w 71 partiach. Weryfikator potwierdził komplet celów. Nie podnoszono
limitów i nie usuwano niezaliczonych przypadków. Lista 27 pominięć z powodami jest
w danych; nie stanowią zaliczonych prób.

Pełne UI na 894: pierwsza próba **345 zaliczonych / 1 błąd / 615 niewykonanych**.
Shard1 241/241; shard2 104/1/135. Test `deny-feedback — junior dark` nie zobaczył karty
w 5 s; snapshot po porażce już ją zawierał. Izolacja tego samego kodu/limitu 5/5,
następnie jednorazowe powtórzenie **całego** shard2 240/240. Shard3 240/240 i shard4 240/240. Zweryfikowano dokładnie 961 różnych ID, bez luk lub nakładania.
Przyczyna opóźnienia **niepotwierdzona**. Oryginalne logi/archiwa nie są nadpisane,
powtarzanych 104 i izolowanych 5 nie doliczamy do unikalnego pokrycia. Globalny komunikat
Playwright „Testing stopped early after 1 maximum allowed failures” to sterowanie,
nie drugi błąd aplikacji. Zapisano lekki sampler zasobów wznowienia; brak OOM nie
ustala przyczyny wcześniejszej porażki. Limity i test pozostały bez zmian.

Starszy pełny UI na 2f3cf3e3 miał 305/1/650, błąd oczekiwania toastu Kreatora; izolacja
2/2 bez zmiany kodu. Ta porażka również pozostaje, nie jest wynikiem nowego źródła.
Zachowane są także wcześniejsze niezaliczenia nowych regresji własnej persony i
przepełnień, po których poprawiono kod przed końcową serią 46.

Całościowy `pnpm format:check` nadal ma kod 1: 16 nietkniętych plików źródłowych,
dwa wygenerowane wyniki testów i dwa przypięte pliki vendora. Zmienione pliki mają
format poprawny. Nie zmieniono ignore, wyjątków lub hashy vendora dla uzyskania zieleni.
Storybook wcześniejszego WIP ukończył build, ale zgłosił DEP0190 i duże chunki;
nie jest to deklaracja braku ostrzeżeń na końcowym źródle.

## Prawdziwy Qwen 8B — wynik ograniczonej próby

Model Qwen3-8B Q4_K_M: SHA256
`d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785`;
llama.cpp b11559, SHA256 serwera
`16c4e5fc3e0a74400033d958722e487a797dbb51c676f5ce1ec9066f1673efd5`.
CPU4/16 GiB, bez GPU. Kolejno, bez konkurujących kompilacji/UI: faktyczne żądanie
przechwycone z Core c502 i jawna ablacja usuwająca wyłącznie nową notę danych.
Historyczne rekordy były syntetyczne; próba przez surowe HTTP poza Core nie wykonywała
akcji biznesowych. Startup60/first-generation60/idle30/turn120 s, kontekst8192,
96tokenów, temperatura0, jedna instancja/slot; bez podniesienia budżetów.

| Wariant | Ładowanie | Pierwsza próba | Pozostałe | Ocena semantyki |
|---|---|---|---|---|
| records_user_data | 26091 ms | HTTP200, timeout60036 ms, 0 tekstu/generacji | 2 niewykonane | NOT_EVALUATED |
| records_ablation | 16904 ms | HTTP200, timeout60137 ms, 0 tekstu/generacji | 2 niewykonane | NOT_EVALUATED |

Oba własne serwery zakończono TERM → po 5 s KILL, kod -9, nie pozostawiono ich do UI.
Pingi nie są postępem. Nie wykonano pełnych 30 lub ponownego 50+50 na nowym źródle.
Nie wolno wnioskować, że większe wagi usunęły #58, że sam CPU jest potwierdzoną
przyczyną, lub że tę semantykę zaliczono jako 0/3 — nie uzyskano odpowiedzi do oceny.
Historyczne V1 0/30 i odrzucony 9B oraz nieoceniony 11B pozostają w poprzednich raportach.
1.5B nadal wyłącznie pomocniczy, główny model8B; docelowy RTX2060 **12GB** nie był tu obecny.

## CI i otwarte karty

CI189 i Windows113 dla **cd55cc86** nadal queued, bez końcowego wyniku. Zachowano
ich head i nie anulowano nowym pushem. W chwili zapisu raportu kandydat nie ma jeszcze
własnego pełnego CI. Po publikacji raportu zaplanowano osobny draft PR do gałęzi PR24,
z osobną grupą concurrency, aby zlecić kontrolę bez anulowania poprzednika; jego numer
i aktualny stan będą zapisane w PR24 oraz checkpoincie. Zielone CI18810/10 i Windows1114/4 dotyczą wyłącznie starszego bfd69b97.
Ich WebView2 63/0/5 i CPU-modelowe pomiary nie stanowią nowego odbioru telefonu/GPU.

| Karty | Co pozostaje |
|---|---|
| #58 | pełny protokół semantyczny na działającym modelu/API; brak niepopartych deklaracji wykonania |
| #22/#41/#48 | jakość wszystkich person, rzeczywiste narzędzia, lokalnie/chmura/hybryda, 8k/16k i RTX12GB |
| #68 | fizyczny telefon/WebView2, dotyk i zasoby na sprzęcie; przyczyna sporadycznego oczekiwania UI |
| #69 | podpis D.0, implementacja transportu/parowania/relay, potem macierz ataków i test telefonu |
| #78 | osadzony backend WebView2, izolacja i zasoby; wybrany rzeczywisty dostawca WWW i testy wyników |
| #77 | zimny/ciepły start i budżety na docelowym GPU; nowe timeouty CPU zachowane |

#69 ma gotowy [pakiet projektu i testów](2026-11-11-remote-design-review.md), ale
brak połączenia. [ADR0021](../ADR/0021-dostep-zdalny-przekaznik-e2e-i-parowanie.md)
wymaga: „Kod powstaje dopiero po podpisie ADR 0019–0023 (zadanie D.0)”. Pytanie
o ten konkretny podpis pozostaje bez odpowiedzi; ogólna zgoda na pełne uprawnienia
nie zatwierdza wybranej architektury kryptograficznej. Zmiana Brokera nadal osobno
do człowieka. Nie rozszerzono dawnych zgód na importy Brokera i dokładną łatkę licencji.

#78: [propozycja lekkiego WebView2](../ADR/0031-wbudowana-przegladarka-webview2.md)
wykorzystuje już dostarczany runtime i osobny profil, bez drugiego Chromium/Electron.
Dzisiejszy BrowserPort nadal uruchamia oddzielny Edge/Chrome przez CDP; embedding
nie jest wdrożony. Dodatki planowane przez istniejące Wasm/WIT. SearchPort pozostaje
bez produkcyjnego dostawcy; net_search jest pomijane w rejestrze, gdy search=None.
Nie wysyłano zapytań do przypadkowych usług ani nie odczytywano kluczy/tokenów CLI.
Pytanie o ścieżkę wyszukiwania nadal bez odpowiedzi.

## Dalsza kolejność i zapis

1. Wykonano: zapis pełnych logów i pierwotnych porażek oraz kontrola niepokrywających się 961 ID UI.
2. Opublikować kandydata, odświeżyć PR i karty bez nieuzasadnionego zamykania.
3. Zachować wynik CI poprzednika, sprawdzić własne pełne CI osobnego draft PR i wykonać
   niezależny przegląd w świeżym kontekście. Dopiero potem rozważać merge.
4. Odtworzyć sporadyczne opóźnienie UI z pomiarem przed/po; bez zwiększania limitów.
5. Na dostępnej maszynie GPU/API dokończyć jakość #58/#41/#48 i zimny/ciepły 8k/16k;
   z osobnym pomiarem rzeczywistej rezerwy VRAM, bez ekstrapolowania z wielkości wag.
6. Po decyzjach wdrożyć #69/#78, wykonać macierze błędów, potem odbiór sprzętowy.

Cloud: Debian13, Node24.22.1, pnpm10.33.0, Rust1.94.1, Playwright1.56.1/Chromium141.
To Linux/headless/fake IPC, bez Windows/WebView2, prawdziwego API, telefonu, GPU lub
mikrofonu. Nie ogłaszamy nieprzerwanej pracy, ukończonej jakości Enterprise ani
usunięcia wszystkich usterek. Nie uruchomiono subagentów, mostów CLI lub nowych
automatyzacji, nie zmieniono chronionych modułów, polityk lub progów. Stara nocna
automatyzacja pozostaje wyłączona. Końcowy checkpoint ma zwolnić dzierżawę i wskazać
wyłącznie sprawdzone własne procesy; praca kończy się po utrwaleniu raportu.

## Dowody do pobrania

[Manifest i instrukcja](artifacts/2026-11-12-records-personas/README.md) obejmują
dokładne komendy, źródła, kody wyjścia, surowe logi, porażki, 27 pominięć Rust,
60 ciał kodeka, próby modelu, cztery części UI i kontrolę unikalnych ID.
Archiwum zachowuje oryginalne bajty; nie zawiera wag modeli ani rzeczywistych sekretów.

[Atlas desktop — e958](artifacts/2026-11-12-records-personas/atlas-desktop-e958.png) ·
[Własna persona w Atlasie320px — 894](artifacts/2026-11-12-records-personas/persona-atlas-320.png) ·
[Adresatka320px — 894](artifacts/2026-11-12-records-personas/persona-recipient-320.png).

Końcowe generatory katalogu/funkcji/HTML/planu, HTML verify, Rustfmt, IPC, wersja,
lint i diff: **wszystkie kod0** na af687d78 z dokumentacyjną kopią roboczą;
[komendy i oryginalne logi](artifacts/2026-11-12-records-personas/validation-manifest.json).
Nie zmieniono programu po16/0/0 ani wejść frontendu po961/961.
