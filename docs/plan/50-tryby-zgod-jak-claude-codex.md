# 50 — Tryby zgód i uprawnień jak w Claude Code i Codex (etap Z)

Stan na **2026-11-03**, wersja 0.0.84. Zlecenie właściciela z 3.11 (dwa zrzuty: menu trybów Claude Code i Codex na
PC): „system zgód i uprawnień ma działać identycznie jak w Claude od Anthropic i Codex od OpenAI — sprawdź dokładnie,
jak to jest w ich wersjach na PC, i tak zrób”. Odpowiedzi na pytania tego samego dnia:

- tryby — „zrób dokładne badanie, przeczytaj ich dokumentację i zrób to u nas na tej samej zasadzie”;
- „Automatycznie” — decyduje **model-sędzia** (jak w Claude i Codex), a dodatkowo **przełącznik „Max dostęp”**
  (chmura/lokalnie) na dole listy: czerwony napis z poświatą; gdy właściciel sam go włączy — **żadnych pytań**;
- **tryb dziecka** — pełne uprawnienia zamknięte na kłódkę, otwarte tylko lekkie; kłódkę otwiera wyłącznie **PIN
  rodzica**; rodzic ma pod PIN-em **pełny dziennik** tego, co robiło dziecko;
- „Pełny dostęp” — **nigdy nie pyta**, jak w Codex (bez zmian względem decyzji z 31.10).

To zastępuje decyzję z 27.10 („bez trybów Akceptuj edycje i Najpierw plan”). Ten dokument to badanie i plan; kodu
etapu Z jeszcze nie ma. Zmiany w Sędzim Cieni (Broker), klasyfikatorze ryzyka i oknie zgód idą na gałąź do
przeglądu właściciela (AGENTS.md).

## 1. Badanie — Claude Code (Anthropic), wersja na PC

Źródło: dokumentacja producenta (permission-modes, permissions; stan z 3.11.2026).

| Tryb (ustawienie) | Bez pytania | Pyta / blokuje |
| --- | --- | --- |
| **Ręcznie** (`default`) | odczyty w katalogach roboczych, wbudowane polecenia tylko do odczytu | edycje plików, polecenia, sieć |
| **Akceptuj edycje** (`acceptEdits`) | jak wyżej + edycje i zwykłe polecenia plikowe (`mkdir`, `mv`, `cp`…) **w katalogu roboczym** | ścieżki chronione, usuwanie ścieżek krytycznych, inne polecenia |
| **Plan** (`plan`) | tylko odczyty i badanie; edycje zablokowane do zatwierdzenia planu | po planie wybór: „Tak, i tryb automatyczny” / „Tak, zatwierdzam ręcznie” / „Nie, planuj dalej” |
| **Automatycznie** (`auto`) | wszystko, ale **każdą akcję ocenia osobny model-klasyfikator** | domyślnie blokuje: `curl \| bash`, wysyłkę wrażliwych danych na zewnątrz, wdrożenia na produkcję, `push --force`, `git reset --hard`, nieodwracalne usuwanie istniejących plików; granice podane przez użytkownika („nie wypychaj”) blokują |
| **Pomijaj uprawnienia** (`bypassPermissions`) | wszystko, także ścieżki chronione | dalej pytają: jawne reguły „pytaj”, usuwanie ścieżek krytycznych (korzeń dysku, katalog domowy, katalog roboczy) |

- **Bezpiecznik trybu automatycznego:** 3 blokady z rzędu albo 20 w sumie — tryb się zatrzymuje i wraca pytanie
  użytkownika; zatwierdzenie wznawia tryb.
- **Okno zgody:** „Czy kontynuować?” — **Tak** · **Tak, i nie pytaj więcej o: `npm test *`** (zakres pokazany
  dokładnie) · **Tak, i przełącz na tryb automatyczny** · **Nie** (z możliwością powiedzenia, co zrobić inaczej);
  Esc — anuluj. „Nie pytaj więcej” zapisuje się na stałe dla polecenia lub domeny w danym projekcie, a dla edycji
  plików — do końca sesji. Opcja trwała pojawia się tylko, gdy okno może pokazać dokładnie, na co się zgadzasz.
- **Reguły** zezwól / pytaj / zabroń (zabroń > pytaj > zezwól), z zakresem: polecenie (prefiks), ścieżka, domena.
- **Ścieżki chronione** (`.git`, `.claude`, `.vscode`, pliki startowe powłoki, `.npmrc`…) nigdy nie są
  zatwierdzane automatycznie poza „Pomijaj uprawnienia”.
- **Wybór trybu** w wersji na PC: przełącznik pod polem pisania, zapamiętany dla folderu (Plan — tylko do końca
  sesji); „Pomijaj uprawnienia” trzeba włączyć świadomie (ostrzeżenie przy pierwszym użyciu); administrator może
  wyłączyć tryb automatyczny i pomijanie uprawnień.

## 2. Badanie — Codex (OpenAI), aplikacja na PC

Źródło: dokumentacja producenta (sandboxing, auto-review, agent approvals & security; stan z 3.11.2026).

| Tryb w aplikacji | Piaskownica + zatwierdzanie | Bez pytania | Pyta |
| --- | --- | --- | --- |
| **Poproś o zatwierdzenie** — „Zawsze pytaj o edycję plików zewnętrznych i dostęp do Internetu” | `workspace-write` + `on-request` | odczyt, edycje w folderze projektu, zwykłe polecenia lokalne | pliki poza folderem projektu, Internet, polecenia wymagające wyjścia z piaskownicy |
| **Zatwierdzaj za mnie** — „Pytaj o zgodę tylko przy potencjalnie niebezpiecznych działaniach” | ta sama piaskownica + **recenzent-model** (`auto_review`) | jak wyżej + to, co recenzent zatwierdzi | to, czego recenzent nie zatwierdzi i agent nie ma bezpieczniejszej drogi |
| **Pełny dostęp** — „Nieograniczony dostęp do Internetu i wszystkich plików na Twoim komputerze” | `danger-full-access` + `never` | wszystko | nic |
| Własny (`config.toml`) | według pliku | — | — |

- **Recenzent** dostaje zwięzły zapis rozmowy (bez ukrytego rozumowania) i proponowaną akcję; może sam wykonać
  sprawdzenia tylko do odczytu. Domyślnie odrzuca: wysyłkę prywatnych danych, sekretów i haseł do niezaufanych
  miejsc; szukanie haseł, tokenów i ciasteczek; trwałe osłabianie zabezpieczeń; działania nieodwracalnie
  niszczące. **To zamiana osoby zatwierdzającej, nie dodatkowe uprawnienie** — recenzent nie poszerza piaskownicy.
- **Po odmowie** agent dostaje powód i polecenie, żeby nie obchodzić blokady, tylko wybrać wyraźnie bezpieczniejszą
  drogę albo zapytać użytkownika. **Bezpiecznik:** 3 odmowy z rzędu albo 10 w ostatnich 50 ocenach — tura
  przerwana. Użytkownik może jednorazowo ponowić odrzuconą akcję (znowu idzie do recenzenta).
- **Ścieżki chronione** w folderze projektu: `.git`, `.codex`, `.agents`, `.aws` — tylko do odczytu, także w
  trybie zapisu.
- **Sieć** w trybie zapisu domyślnie wyłączona; prośba o sieć = zatwierdzenie.
- Na Windows — natywna piaskownica (blokuje zapis poza folderem i sieć bez zgody).
- Zgody mają zakres: **raz** albo **na sesję**; zalecany najwęższy, który pozwala pracować dalej. Reguły prefiksów
  poleceń: zezwól / pytaj / zabroń.

## 3. Wspólna zasada obu programów

1. **Odczyt w folderze pracy jest wolny**, zmiany i wyjście na zewnątrz (pliki poza folderem, Internet) są
   granicą.
2. **Tryb „pośredni” — zmiany w folderze bez pytania** (Claude: Akceptuj edycje, Codex: Poproś o zatwierdzenie).
3. **Tryb „automatyczny” — zatwierdza model, nie człowiek** (Claude: klasyfikator, Codex: recenzent); twarde
   kategorie odmowy, bezpiecznik po serii odmów, powrót do człowieka.
4. **Pełny dostęp — świadomie włączany, bez pytań.**
5. **Okno zgody z zakresem** („raz”, „nie pytaj więcej o dokładnie to”, „przełącz tryb”, „nie — zrób inaczej”).
6. **Ścieżki chronione i krytyczne** nie są zatwierdzane automatycznie poza pełnym dostępem.

## 4. Projekt dla Alfy

### 4.1 Menu „Tryb pracy” (pod polem pisania, jak w obu programach)

| # | Nazwa w Alfie | Odpowiednik | Bez pytania | Pyta | Poziom Sędziego |
| --- | --- | --- | --- | --- | --- |
| 1 | **Plan** — „Najpierw plan, zmiany po Twojej akceptacji” | Claude Plan | odczyt, badanie | po planie: „Tak, i działaj automatycznie” / „Tak, zatwierdzam ręcznie” / „Nie, planuj dalej” | L0 + przebieg planu |
| 2 | **Ręcznie** — „Zawsze pytaj przed zmianami” | Claude Ręcznie | odczyt w folderze pracy | każda zmiana, polecenie, sieć | L1 |
| 3 | **Akceptuj edycje** — „Zmiany w folderze pracy bez pytania; pliki poza nim i Internet — pytaj” | Claude Akceptuj edycje = Codex Poproś o zatwierdzenie | odczyt, edycje i zwykłe polecenia w folderze pracy | pliki poza folderem, Internet, ścieżki chronione, usuwanie trwałe | **nowy** (L2 — dziś „Pytaj o ryzykowne”) |
| 4 | **Automatycznie** — „Sędzia Cieni decyduje; pyta tylko o niebezpieczne” (domyślny) | Claude Automatycznie = Codex Zatwierdzaj za mnie | wszystko, co zatwierdzi **model-sędzia** | to, czego sędzia nie zatwierdzi; po 3 odmowach z rzędu albo 20 w zadaniu — pyta właściciela | L3 + model-sędzia |
| 5 | **Pełny dostęp** — „Nieograniczony dostęp do Internetu i wszystkich plików” (pomarańczowy, jak w Codex) | Claude Pomijaj uprawnienia = Codex Pełny dostęp | wszystko | **nic** (decyzja 31.10); zostają twarde blokady Jądra, STOP i dziennik | L4 |

Pod listą — **przełącznik „Max dostęp”** (czerwony napis z poświatą, oddzielony linią): włączony przez właściciela
= żadnych pytań niezależnie od trybu, z wyborem **modele lokalne / modele w chmurze / wszystkie** (do potwierdzenia
w §7 — tak rozumiem „Chmura/Lokalnie”). Włączenie: jedno potwierdzenie w oknie Sędziego (jak dziś L4), stan
pamiętany po restarcie, stały wskaźnik w belce. W trybie dziecka — kłódka (§4.4).

Klawisze 1–5 wybierają tryb (jak dziś 1–4); „Plan” obowiązuje do końca rozmowy, pozostałe są globalne i pamiętane.

### 4.2 Okno zgody (karta Sędziego) — opcje jak w Claude i Codex

Pytanie „Czy kontynuować?” z dokładnym opisem akcji (polecenie, ścieżka, host) i przyciskami:

1. **Tak** (raz);
2. **Tak, i nie pytaj więcej o: `<dokładny zakres>`** — zakres pokazany słowami („polecenie `npm test`”, „pisanie
   w folderze `E:\Projekt`”, „strona `api.github.com`”); tylko gdy okno potrafi pokazać pełny zakres; zapis: polecenia
   i strony — na stałe dla folderu pracy, edycje — do końca rozmowy (jak w Claude); lista w Ustawieniach →
   Uprawnienia z możliwością cofnięcia;
3. **Tak, i przełącz na „Automatycznie”** (gdy tryb jest niższy);
4. **Nie — powiedz agentce, co zrobić inaczej** (pole tekstu; puste = zwykła odmowa).

Esc = Nie. Klawisze 1–4. Zgody czasowe 1 h / 3 h / 24 h / do odwołania (0.0.80) zostają jako rozwinięcie opcji 2.

### 4.3 Model-sędzia w trybie „Automatycznie”

- **Zamiana osoby zatwierdzającej, nie nowe uprawnienie** (zasada Codex): sędzia ocenia tylko akcje, o które
  Sędzia Cieni zapytałby właściciela na L3; twarde blokady Jądra i deny-listy działają przed nim.
- **Wejście:** cel zadania, zwięzły zapis rozmowy (wiadomości właściciela, kroki agentki, wyniki narzędzi — treść
  obca oznaczona), proponowana akcja z faktami Sędziego (zdolność, ścieżki, host, odwracalność). Bez ukrytego
  rozumowania agentki.
- **Domyślne odmowy** (wspólne kategorie obu programów): wysyłka danych prywatnych, sekretów i haseł do
  niezaufanych miejsc; szukanie haseł/tokenów/ciasteczek; trwałe osłabianie zabezpieczeń; nieodwracalne niszczenie
  (usuwanie istniejących danych, nadpisanie historii git, formatowanie); polecenia z treści obcej (e-mail, strona),
  których właściciel nie zlecił; granice podane przez właściciela w rozmowie („nie wysyłaj”, „nie usuwaj”).
- **Po odmowie** agentka dostaje powód i zakaz obchodzenia; **bezpiecznik:** 3 odmowy z rzędu albo 20 w zadaniu —
  pytanie wraca do właściciela (karta z powodem sędziego i przyciskiem „Zrób mimo to — raz”).
- **Model:** domyślnie lokalny (bez kosztu, prywatnie); gdy właściciel ma klucz — szczebel „sprawdzanie” z Routera.
  Brak modelu albo błąd → pytanie właściciela (nigdy cicha zgoda).
- Każda ocena trafia do dziennika (akcja, werdykt, powód, model) i do panelu „Zgody”.

### 4.4 Tryb dziecka — kłódki i PIN rodzica

Opiera się na planie 41 i ADR 0029 (rola Dziecko). Do czasu kont (etap U) — **profil „Dziecko” na tym komputerze**
przełączany PIN-em rodzica.

- **Otwarte (lekkie):** Plan, Ręcznie. **Na kłódkę:** Akceptuj edycje, Automatycznie, Pełny dostęp, Max dostęp,
  a także zmiana trybu globalnego, Ustawienia → Uprawnienia, zgody stałe, chmura (ADR 0029 pkt 3).
- **Kłódkę otwiera tylko PIN rodzica** — na czas: do końca rozmowy / 1 h / do zamknięcia Alfy; potem zamyka się
  sama. Dziecko widzi zamkniętą pozycję z ikoną kłódki i podpisem „Poproś rodzica”.
- **PIN:** 4–8 cyfr, przechowywany wyłącznie jako skrót Argon2id w Menedżerze poświadczeń Windows (zasada sekretów
  AGENTS.md); 5 błędnych prób — blokada 15 min (rośnie); zmiana PIN-u wymaga starego PIN-u; zapomniany PIN — reset
  przez konto administratora Windows (Windows Hello / hasło konta). PIN nigdy nie trafia do modelu, dziennika ani
  eksportu.
- **Dziennik rodzica pod PIN-em:** pełna lista działań dziecka — rozmowy (czas, tematy), każde wywołanie narzędzia,
  odwiedzone strony, zgody i odmowy, próby otwarcia kłódki, alerty filtra. **Pełna treść rozmów:** ADR 0029 pkt 2
  mówi „domyślnie tematy i alerty, treść po jawnym włączeniu, dziecko widzi znak”; właściciel 3.11: „pełne logi”
  — do potwierdzenia w §7, czy pełna treść ma być domyślnie włączona.

### 4.5 Ścieżki chronione i krytyczne w Alfie

Jak w obu programach: `.git`, `.alfa`, pliki konfiguracji Alfy, profil przeglądarki, pliki startowe powłoki, `.npmrc`,
`.aws`, `.ssh` — w trybach 2–4 nigdy bez pytania (w 4 — do sędziego tylko odczyt). Usuwanie korzenia dysku, katalogu
domowego, katalogu roboczego — twarda odmowa Jądra (już jest; Pełny dostęp też nie pomija).

## 5. Etapy

| Nr | Zadanie | Gdzie | Zależy od |
| --- | --- | --- | --- |
| Z.0 | Ten plan + decyzje właściciela (§7) | dokumenty | — |
| Z.1 | Menu 5 trybów + przełącznik Max dostęp (wygląd: czerwień z poświatą, kłódki), we wszystkich stylach i motywach | UI | Z.0 |
| Z.2 | Poziom „Akceptuj edycje” w Sędzim (zakres = folder pracy, ścieżki chronione) | Broker — gałąź do przeglądu | Z.0 |
| Z.3 | Okno zgody z 4 opcjami i zakresem „nie pytaj więcej” | broker-ui — gałąź do przeglądu | Z.2 |
| Z.4 | Model-sędzia dla „Automatycznie” + bezpiecznik + dziennik | nowy moduł `approval-judge` + Broker | Z.2 |
| Z.5 | Plan jako tryb (przebieg planu i trzy odpowiedzi) | agent-runtime + UI | Z.1 |
| Z.6 | Profil Dziecko z kłódkami i PIN-em (Credential Manager, Argon2id) | Broker + UI — gałąź do przeglądu | Z.1, Z.2 |
| Z.7 | Dziennik rodzica pod PIN-em | app-api + UI | Z.6 |
| Z.8 | Testy: każdy tryb × każda kategoria akcji; ataki (wstrzyknięcie przez treść obcą do sędziego, zgadywanie PIN-u, obejście kłódki z palety/skrótów/głosu); prawdziwa aplikacja w każdym stylu i motywie | testy | wszystkie |

## 6. Bezpieczeństwo (lista kontrolna)

- Sędzia jako model może się mylić (obie dokumentacje tak piszą) — dlatego działa **tylko** na L3, po twardych
  blokadach, z bezpiecznikiem i dziennikiem; nie jest granicą bezpieczeństwa.
- Treść obca w wejściu sędziego oznaczona jako dane (wstrzyknięcie poleceń do sędziego = osobny test).
- Max dostęp i Pełny dostęp: włączane tylko przez właściciela (okno Sędziego, nie agentka, nie głos z niepewnym
  mówcą); w profilu Dziecko niemożliwe bez PIN-u.
- PIN: tylko skrót, limit prób, brak w logach; kłódka sprawdzana w Sędzim, nie tylko w interfejsie.

## 7. Decyzje właściciela (3.11)

1. **Max dostęp:** jeden czerwony przełącznik z wyborem **modele lokalne / modele w chmurze / wszystkie**.
2. **Dziennik rodzica:** jak w ADR 0029 — pod PIN-em działania, tematy i alerty; pełną treść rozmów rodzic włącza
   jawnie dla dziecka (dziecko widzi stały znak).
3. **Dziecko bez PIN-u:** tylko **Plan** i **Ręcznie**; reszta (Akceptuj edycje, Automatycznie, Pełny dostęp,
   Max dostęp) na kłódkę.
4. Model-sędzia w „Automatycznie”, „Pełny dostęp” nigdy nie pyta, menu na zasadzie obu programów (§4.1) —
   przyjęte; zastępuje decyzję z 27.10.

## 8. Stan wykonania (3.11 wieczór, gałąź `do-przegladu-tryby-zgod`)

- **Z.2 zrobione (deaaf0a1 + poprawki z przeglądu):** `DeclaredFacts.workdir` z kontekstu wywołania; Sędzia liczy
  `outside_workdir` (ścieżka poza profilem albo poza folderem pracy rozmowy; brak folderu = poza) i `protected_path`.
  Reguły `OutsideWorkdir` (≤ L2, „zawsze zezwalaj” możliwe) i `ProtectedPath` (≤ L3, nigdy stała zgoda). Testy:
  tabela 89 × 5, własności z nowymi faktami, `safety-broker-contract/tests/workdir.rs`.
- **Z.1 zrobione (3.11 wieczór, §8.2):** menu ma 5 trybów (klawisze 1–5); nazwy jak w obu programach: Plan (L0),
  Ręcznie (L1), Akceptuj edycje (L2), Automatycznie (L3), Pełny dostęp (L4) i czerwony przełącznik „Max dostęp”.
- **Przegląd w świeżym kontekście (3.11):** nic krytycznego, brak regresji (nowe fakty tylko dodają pytania).
  Luki nowej ochrony i stan:
  1. polecenie powłoki w folderze pracy, które pisze do `.git\hooks` czy `.npmrc`, ocenia się po katalogu
     roboczym — **poprawione** (72214cc2): słowa polecenia wskazujące ścieżkę chronioną idą do Sędziego jako
     osobny zakres; zostaje zapis pośredni (np. `git commit` pisze do `.git` bez podania ścieżki) — jak w Codex
     wymagałby piaskownicy zapisu, do decyzji;
  2. aliasy 8.3 (`GIT~1`) — **poprawione** (dopasowanie jak w deny-listach);
  3. usuwanie katalogu nadrzędnego wobec `.git` (rekurencyjne) nie jest „ścieżką chronioną” — **otwarte**
     (Sędzia ocenia ścieżki leksykalnie; na L2 i tak pyta reguła usuwania);
  4. dowiązania (junction) w folderze pracy prowadzące poza niego — **poprawione** (4.11 noc): narzędzia
     plikowe rozwiązują dowiązania przy każdym wywołaniu (`tools_common_contract::workdir_links`); ścieżka,
     która przez dowiązanie wychodzi poza folder pracy, idzie do Sędziego bez folderu pracy — L2 pyta.
     Zostają polecenia powłoki (cel ścieżki oceniany leksykalnie, jak zapis pośredni z pkt 1);
  5. bez folderu pracy (mosty CLI, `run_code`, wtyczki, MCP) L2 pyta o każdą zmianę jak L1 — świadomie (bezpieczniej);
  6. folder pracy pod katalogiem o chronionej nazwie (np. `.claude\worktrees`) — **poprawione**: liczą się tylko
     człony poniżej folderu pracy; lista uzupełniona (`.cargo`, `.github\workflows`, `.gitattributes`, `.zshenv`,
     `.envrc`, `.devcontainer`, `.hg`, Autostart).

### 8.1 Projekt „Max dostęp” (następny krok, Broker — do przeglądu)

- **Skąd Sędzia wie, czy model jest lokalny:** `ToolCtx.model_location` (`local` / `cloud`) ustawia agent-runtime
  z dostawcy przebiegu (llama.cpp / Ollama / LM Studio = lokalny; klucz API albo most CLI = chmura); trafia do
  `DeclaredFacts` tak jak folder pracy — z kontekstu wywołania, nigdy od modelu. Brak informacji — ani
  „lokalne”, ani „chmura”; obejmuje go tylko „wszystkie” (zmiana 4.11 po przeglądzie).
- **Stan w Sędzim:** `MaxAccess { off, local, cloud, all }` zapisywany jak poziom globalny (raz potwierdzony w oknie
  Sędziego, pamiętany po restarcie, obniżenie od razu). Gdy obejmuje pochodzenie modelu przebiegu — poziom
  skuteczny = L4 (twarde blokady Jądra, STOP i dziennik zostają).
- **Interfejs:** pod listą trybów czerwony napis z poświatą „Max dostęp” + wybór lokalne / chmura / wszystkie;
  znak w belce jak dziś „Pełny dostęp”; w profilu Dziecko kłódka (Z.6).
- **Testy:** tabela (poziom × pochodzenie × ustawienie), własność „bez informacji o modelu Max dostęp lokalny
  nigdy nie podnosi”, prawdziwa aplikacja w każdym stylu i motywie.

### 8.2 Stan 3/4.11 noc (gałąź `do-przegladu-tryby-zgod`, ad9ddcba…c07c72fe)

- **Z.1 Max dostęp:** cel `Models{lokalne|chmura|wszystkie}` w Sędzim, pochodzenie modelu tury
  (`ToolCtx.model_location`, z dostawcy przebiegu — nazwie modelu ufa tylko u Routera), zapis po restarcie,
  polecenie `permissions_request_max_access`, przełącznik i czerwony znak w belce.
- **Z.3 okno zgody:** „Zezwól i przełącz na „Automatycznie”” (tylko pojedyncza akcja, tryb globalny < L3, podmiot
  bez własnego poziomu, bez niezaufanej treści i bez kłódki rodzica); rdzeń przyjmuje to przełączenie jako
  zapamiętany poziom (znacznik w Sędzim, kasowany każdą inną zmianą globalną). Opcja 4 — po odmowie „Powiedz
  agentce, co zrobić inaczej” (wskazówka do trwającego zadania albo do pola pisania).
- **Z.5 Plan:** na L0 agentka ma tylko narzędzia odczytu i zasadę planu w prompcie; plan kończy przebieg bez
  weryfikacji; pod planem trzy odpowiedzi (tak, automatycznie / tak, ręcznie / nie, planuj dalej). Plan obowiązuje
  otwartą rozmowę.
- **Z.6 profil Dziecko:** sufit L1 w tabeli autonomii Sędziego (obejmuje zapamiętany Pełny dostęp i Max dostęp);
  przy zamkniętej kłódce podniesienia i Max dostęp odrzucane, bez zgód stałych i bez przełączenia z okna. PIN 4–8
  cyfr, tylko skrót Argon2id (19 MiB, 2 przejścia) z solą z CSPRNG; 5 błędnych prób — blokada 15 min, każda
  następna dwa razy dłuższa (do 16 h), licznik przetrwa restart; uszkodzony zapis — kłódka zamknięta. Otwarcie na
  15 min / 1 h / do zamknięcia Alfy; zamknięcie bez PIN-u. Interfejs: kłódki i „Poproś rodzica” w menu trybów,
  znak w belce, karta w Ustawienia → Uprawnienia.
  **Odstępstwo do decyzji:** skrót PIN-u leży w pliku `child-lock.json` w prywatnym katalogu Sędziego (ścieżka
  Jądra, agentki nie mają dostępu), nie w Menedżerze poświadczeń — Sędzia jako usługa działa na osobnym koncie;
  przeniesienie do Menedżera poświadczeń tego konta — przy przeglądzie. Poza usługą (tryb przenośny, w procesie)
  plik należy do konta użytkownika, więc ochrona jest umowna — karta profilu mówi to wprost
  (`tamper_resistant`); decyzja właściciela: dopuścić z ostrzeżeniem czy wymagać Sędziego jako usługi.
- **Przegląd Z.6 w świeżym kontekście (4.11):** nic krytycznego; poprawione wszystkie poważne: zamknięcie kłódki
  (także upływ terminu, włączenie profilu) unieważnia tokeny, plany i karty wydane bez niej, karta zatwierdzona po
  zamknięciu nie podnosi trybu, nie daje zgody stałej ani przełączenia; otwarcie, wyłączenie i nowy PIN obowiązują
  dopiero po zapisie w Audycie; „do zamknięcia Alfy” kończy się, gdy rozłączy się ostatnie połączenie aplikacji
  (także po awarii). Z drobnych: samopodniesienie agentki przy kłódce liczone jako blokada Jądra, obniżenie
  `SessionAgent` rozpoznawane bez sufitu, Max dostęp w metrykach tylko gdy obowiązuje, PIN zerowany w pamięci,
  zapis pliku z `sync_all`, interfejs odświeża stan po upływie otwarcia, atrapa z rosnącą blokadą. Stan
  „nieznany” profilu przy zerwanym łączu — **poprawione** (4.11 noc): karta mówi „stan nieznany” i daje tylko
  ponowny odczyt, menu trybów zachowuje ostatni znany stan kłódki. Otwarte: zmiana zegara systemowego (tylko
  administrator) wpływa na terminy.
- **Z.7 dziennik rodzica (wersja 1):** po PIN-ie (ta sama blokada prób) 20 ostatnich rozmów z tematem i czasem
  oraz działania z osi czasu (narzędzia, Audyt). Pełna treść rozmów — jeszcze nie (włączana jawnie przez
  rodzica, ze stałym znakiem dla dziecka); „do końca rozmowy” jako czas otwarcia — jeszcze nie.
- **Przegląd w świeżym kontekście (3.11, zakres Z.1/Z.3/Z.5):** nic krytycznego; 3 poważne poprawione w 29946562
  (strażnik nie cofa przełączenia przy czekającej prośbie, znacznik zamiast każdego wpisu bez terminu, pochodzenie
  „lokalne” z nazwy modelu tylko u Routera), z drobnych poprawione: kolejność Audytu przy przełączeniu, brak
  opcji po niezaufanej treści. Błąd łącza z Sędzią dawał w kliencie zdalnym poziom L0 (agentka nagle w trybie Plan) —
  **poprawione** (4.11 noc): ostatni poziom odczytany dla sesji i agentki, najwyżej L1, bez odczytu L0.
  Zasięg „chmura” obejmował też działania bez
  modelu — **poprawione** (4.11 noc): „chmura” = tylko przebiegi z modelem w chmurze (mosty CLI oznaczone
  jawnie), działania bez informacji o modelu (wtyczki, zrzut ekranu, MCP) obejmuje tylko „wszystkie”.
- **Edge case:** restart Sędziego przed przyjęciem przełączenia przez rdzeń (≤ 15 s) — przełączenie przepada,
  obowiązuje poprzedni zapamiętany poziom (bezpieczny kierunek).

## Źródła

- Claude Code — Permission modes: https://code.claude.com/docs/en/permission-modes
- Claude Code — Configure permissions: https://code.claude.com/docs/en/permissions
- Codex — Sandboxing: https://learn.chatgpt.com/docs/sandboxing
- Codex — Auto-review: https://learn.chatgpt.com/docs/sandboxing/auto-review
- Codex — Agent approvals & security: https://learn.chatgpt.com/docs/agent-approvals-security
