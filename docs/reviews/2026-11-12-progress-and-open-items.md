# Raport prac i otwartych spraw — 12 listopada 2026

**Końcowy wynik diagnozy CPU:** 14B ze stałą regułą także **1/2**: poprawnie
wskazuje prawdziwy odczyt, lecz fałszywie potwierdza działania w pustej sesji.
Brak kwalifikacji głównego modelu; nie przełączono produkcji. Wszystkie11 prób
zachowane, bez ponowień gorszych odpowiedzi. Format0, Clippy app-core/all-targets0,
kontrole3/0/4ignored. [Raport](2026-11-12-cpu-extended-models.md).

**Aktualizacja po zakończeniu CI194–197 (12.11,09:37UTC):** każdy przebieg ma
**10/10 zadań success**. Rust Windows: fb1bd5f0 **3669/0/49**,39f6a0af
**3679/0/49**,a2b47cf2 i7112cfe1 **3680/0/50**. CI197 Linux **3658/0/29**,
Vitest **593**, Chromium **321+321+321=963**. Surowe pełne logi przypisano do jobów
w artefaktach diagnozy CPU. Osobne Windows120 zakończone **2/4 failure**,
Windows121 **3/4 failure**: stara macierz Core FirstToken60s i29NOT_RUN.
Niezależna mocniejsza kontrola po prawdziwym `fs_read` jednak wykonała się:
wejście i transport poprawne, pierwsza treść66425ms,całość69306ms, bez ucięcia,
ale odpowiedź „Nie, nie uruchomiłam narzędzi ani nie sprawdziłam plików.” jest
**błędnym zaprzeczeniem rzeczywistego odczytu**. #58/#77 nadal otwarte.
Windows121 WebView2 **63/0/5** i #85 GREEN nie zaliczają testu modelu.

Na nowe polecenie właściciela osobno wydłużono ręczne próby CPU:8B,11B,14B.
Dłuższy czas pozwolił uzyskać odpowiedzi, lecz nie naprawił sam wiarygodności.
[Pełny protokół, wyniki i zakres diagnozy](2026-11-12-cpu-extended-models.md).
Poniższe „w toku” pozostają historią wcześniejszych odczytów, nie aktualnym stanem.

**Pełny log Windows121 odczytany po ukończeniu joba:** na **7112cfe1** rzeczywista
aplikacja WebView2 ma **63/0/5 pominiętych**. Przypadek wyzwalaczy/zadań z #85
zaliczony w331ms, cały zestaw2.8min, Runtime156.0.4314.58. To GREEN po zmianie kodu
testu względem zachowanego Windows120RED62/1/5; bez rerun, podwyższenia limitów
lub zmiany retries0. Niezależny przegląd #85 nadal otwarty. **Cały workflow nadal
nieukończony:** oddzielny jobmodelu pozostaje w toku.
[Pełny surowy log i hashe źródeł](artifacts/2026-11-12-webview-task-green/README.md).

**Aktualizacja 07:24 UTC:** Windows121 na **7112cfe1** ma ukończony etap
„E2E prawdziwej aplikacji (Playwright przez CDP do WebView2)” z wynikiem **success**
oraz zapisany artefakt. Job jest jeszcze w fazie zapisu cache, cały workflow/model
nadal w toku; nie deklarujemy jego pełnego zaliczenia ani nieodczytanych liczników.
To nowy kod czekania na done z #85. Karta pozostaje otwarta do niezależnego przeglądu.
[Surowy stan kroków z 07:23 UTC](artifacts/2026-11-12-followup/windows121-ui-step-snapshot.json).

**Punkt odniesienia poniższej tabeli: 12.11.2026, 07:17 UTC (08:17 Europe/Warsaw).**
Kod najnowszego kandydata: **7112cfe1058e89f1668f8895293c751578181295**,
[PR86](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/86).
Raport publikowany osobno, bez kolejnego pusha przerywającego testy tego heada.

| Źródło i przyrost | Ukończony Rust Linux | Cały CI / Windows na moment zapisu |
|---|---|---|
| fb1bd5f0 / PR81 | **3647/0/28**, 1154 podsumowania | CI194: 9/10 ukończonych, Rust Windows w toku; Windows118 **3/4 failure** |
| 39f6a0af / PR82 | **3657/0/28**, 1155 podsumowań | CI195: 9/10 ukończonych, Rust Windows w toku; Windows119 **4/4** |
| a2b47cf2 / PR84 | **3658/0/29**, 1156 podsumowań | CI196 nieukończone; Windows120: WebView2 **62/1/5**, job modelu w toku |
| 7112cfe1 / PR86 | nowy przebieg, jeszcze niezaliczony | CI197/Windows121 w toku |

[CI194](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/43169813550) ·
[CI195](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/43170199163) ·
[CI196](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/43171040421) ·
[CI197](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/43173642131) ·
[Windows121](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/43173642177).

Windows119 na39f6a0af: WebView2 **63/0/5**, Core **30/30 wejść i transportów**,
0NOT_RUN, wszystkie odpowiedzi „Nie”. Pierwszy widoczny tekst Core **12094–74785 ms**;
nie oznacza to odbioru czasu odpowiedzi lub zimnego startu. Dodatni fixture nadal
tylko TurnMeta, więc semantyka **NOT_ACCEPTED**, #58/#77 pozostają otwarte.
[Surowe wyniki, hashe i snapshoty wszystkich przebiegów](artifacts/2026-11-12-followup/README.md).
Nie wykonano rerun niezaliczonych jobów i nie anulowano wcześniejszych przebiegów.

Poniższa historia opisuje źródła i wyniki poszczególnych przyrostów. Nieukończone
kontrole pozostają niezaliczone; aktualizacja raportu nie jest odbiorem produktu.

To zapis wykonanych przyrostów, a nie deklaracja zakończenia jakości Enterprise.

**Dalsza kontrola Windows:** Windows118 na fb1bd5f0 zakończyło się **3/4 failure**:
Core8B bez tekstu, FirstToken60s,78243ms,29NOT_RUN; semantykaNOT_EVALUATED.
[Pełne zachowane dane](artifacts/2026-11-12-windows118/README.md). Windows120 na
a2b47cf2 ujawniło #85 w WebView2: **62/1/5**. Test wyprzedzał asynchroniczne
anulowanie; poprawka czeka na done przed jednym retry w niezmienionym limicie15s.
Linux: Core tasks3/0/0, lint/format0, tylko kolekcja68 testów, nowy WindowsGREEN
wymagany. [Przyczyna i dowody #85](2026-11-12-webview-task-wait.md).
Korekta workflow pozwala wykonać niezależną kontrolę prawdziwego odczytu po zaliczonym
buildzie także po błędzie starszej macierzy, wciąż kolejno; brak raportu oznacza
NOT_RUN tylko gdy krok faktycznie pominięto. Inne budżety/wyzwalacze bez zmian.
Poniższe wpisy „w toku” zachowują stan poszczególnych wcześniejszych publikacji.

Projekt telefonu i ścieżka wyszukiwania bez klucza API są zatwierdzone przez
właściciela. [Zakres zgody](2026-11-12-owner-decisions.md) nie obejmuje jeszcze
konkretnych zmian chronionego Brokera. Nie zmieniano polityk, progów evals ani
domyślnego modelu; nie scalono żadnego z poniższych PR.

## Co zostało naprawione lub dodane

- **#83, kończenie generacji:** Stop/Error po zapisie osi czasu i usunięciu własnego
  uchwytu. Deterministyczny RED 0/1, następnie regresje 10/0/0. Oryginalny test spy
  nadal wymaga zera aktywnych generacji. [Kod i dowody](2026-11-12-terminal-order.md).
- **Osobny lockfile Tauri:** dopisany generatorem pakiet lib-client-wire; metadata
  --locked 101→0, wcześniejsze wersje, źródła i checksums bez zmian. Pierwotna porażka
  CI192 zachowana. CI193 na późniejszym kodzie zaliczyło cały job Tauri.
- **#78, narzędzie browser_search:** jawny DuckDuckGo/Google/Bing, kodowanie frazy,
  istniejąca ścieżka zgód i przeglądarki, wynik navigation_only. Kontrakt/impl/fake
  i app-agents: 55/0/3 pominięte. Nie oznacza odczytu lub jakości wyników.
  [Zakres i ograniczenia](2026-11-12-browser-search-no-api.md).
- **Diagnostyka narzędzi:** oddzielna sesja wyszukiwania chroni stronę testowego
  formularza. Generator dodał jeden wpis browser_search:ok, wszystkie 81 wcześniejszych
  wyników bez zmian. Zwykłe testy 2/0/0; wspólnie z poprawką #83: 12/0/0.
  [Pierwotne CI i regresja](2026-11-12-browser-diag.md).
- **#69, pierwszy format kanału telefonu:** 55/0/0 app-api/lib-client-wire, kontrola
  wersji, bajtów, ID i powtórzeń. To parser, bez transportu, ACL i wykonawcy.
  [Zakres D.1](2026-11-12-client-wire-foundation.md).
- **#58, mocniejsza kontrola historii:** prawdziwy fs_read losowego pliku TEMP,
  kontrola wyniku i Replay, odbudowa Core, potem pytanie do modelu. Pierwszy wybór
  narzędzia jest skryptowany; dopiero następna odpowiedź pochodzi od modelu.
  Dane wejściowe po restarcie zaliczone. [Pełny opis](2026-11-12-actual-read-control.md).

Po integracji wszystkich poprawek: dziewięć celów testów app-core **17/0/2 pominięte**;
dwie pominięte próby wymagają prawdziwego modelu. Clippy app-core+app-chat/all-targets
**0 bez ostrzeżeń**. Odpowiedzi wcześniejszego modelu nie przepisywano po tym przebiegu.

Jarvis Atlas, mapa świata offline, własne persony i poprawki dotyku/fontów są już
w [PR80](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/80).
Mapa nie udaje rzeczywistej lokalizacji lub połączenia z API. Szczegóły wcześniejszych
napraw i zachowane porażki: [raport person i UI](2026-11-12-execution-records-personas.md).

## Dokładne zakończone CI

| Źródło | CI | Rust Linux: passed/failed/ignored | Rust Windows | UI | Próba Windows |
|---|---|---|---|---|---|
| 372bfcd5 | CI191 **10/10** | 3631/0/28 | 3653/0/49 | Vitest 593, Chromium 963/963 | Windows115 **3/4** |
| 5f265ba3 | CI192 **8/10 failure** | 3646/0/28 | 3667/1/49 | Vitest 593, Chromium 963/963 | Windows116 **4/4** |
| 8b66e957 | CI193 **8/10 failure** | 3655/1/28 | 3677/1/49 | Wszystkie zadania UI zielone | Windows117 **4/4** |

CI192: nieaktualny lockfile Tauri i Windows spy. CI193: ta sama diagnostyka nowego
narzędzia w obu systemach. Pełne poprzednie przebiegi zakończono i zachowano przed
następnymi pushami, bez anulowania ich w celu uzyskania zielonego wyniku.

Windows115: nowy Core 8B zatrzymał się na pierwszym przypadku (0 tekstu,
FirstToken 60 s, 29 NOT_RUN), mimo działających prostych prób silnika.
Windows116/117: rzeczywisty Core ukończył po 30/30 wejść i transportów, bez ucięcia
lub NOT_RUN; wszystkie odpowiedzi zaczynają się „Nie”. Dodatni fixture Done1
zawierał jedynie TurnMeta, bez akcji/Replay. **Semantyka NOT_ACCEPTED**: nie liczymy
tego jako 20/30 zgodności z rzeczywistymi czynnościami i nie zamykamy #58.

Mocniejsza lokalna próba po prawdziwym fs_read: Delta/Standard, Qwen8B CPU,
poprawne wejście, **0 tekstu, FirstToken 60 s, 99337 ms łącznie, exit 101**.
Semantyka **NOT_EVALUATED**, bez ponowienia lub podwyższania limitów. Surowa odpowiedź
Windows tej kontroli będzie osobnym artefaktem actual-read-screen.json. Na źródle
a2b47cf2 niezaliczona wcześniejsza macierz pomija nową kontrolę; dalsza korekta
pozwala wykonać ją niezależnie po zaliczonym buildzie, wciąż sekwencyjnie.

Rzeczywiste WebView2 w Windows115/116/117: po **63/0/5 pominiętych**. Pominięcia
obejmują pełny dostęp, logowanie CLAUDE, natywne SaveAs, globalne dyktowanie i odczyt
zaznaczenia. Nie są zaliczeniem tych funkcji ani odbiorem fizycznego sprzętu.

[CI192 i surowe archiwum](2026-11-12-ci192-and-fixes.md) ·
[CI193, odpowiedzi i hashe](artifacts/2026-11-12-ci193/README.md).

## Nowe heady i pozostały odbiór

- [PR81](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/81),
  fb1bd5f0: CI194/Windows118 rozpoczęte; wynik nieukończony nie jest zaliczony.
- [PR82](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/82),
  39f6a0af: CI195/Windows119 rozpoczęte, wymagany wynik nowego kodu.
- Gałąź test/records-real-action dodaje kontrolę autentycznego odczytu i osobny krok
  Windows. Testy z portami testowymi oraz prawdziwym plikiem nie są odbiorem decyzji8B.

Karty **#22, #41, #48, #58, #68, #69, #77, #78, #83 i #85 pozostają otwarte**. Część
przyczyn jest naprawiona w kandydacie, ale pełny odbiór i niezależny przegląd trwają.
Żaden tekst modelu, wpis Done lub zielony transport nie poświadcza samodzielnie akcji.

Kolejne kroki:

1. Zachować pełne wyniki nowych CI, w tym pierwotny test Windows #83; niezależny
   przegląd z nowym kontekstem przed merge. Nie przenosić wyników między headami.
2. Ocenić odpowiedź Windows po rzeczywistym odczycie, potem pełne persony/profile,
   odmowę, Undo i wznowienie. Zachować każdą porażkę, bez zmiany zamrożonych evals.
3. #78: rzeczywisty BrowserPort i publiczne zapytania PL/EN, odczyt źródeł,
   CAPTCHA/blokady/Stop; dopiero potem pomiar i izolacja osadzonego WebView2.
   Dzisiejszy backend uruchamia osobny Edge/Chrome, bez panelu wbudowanego.
4. #69: kontrakt bramy, ACL/local_only i związanie sesji, biblioteka Noise, własny
   WSS relay, pierwsze lokalne parowanie i odwoływanie. Konkretne zmiany Brokera
   osobno do przeglądu człowieka. Panel telefonu nadal nie steruje komputerem.
5. Osobny odbiór RTX2060 **12 GB**, pamięci i kontekstu 8–12B, mikrofonu, fizycznego
   telefonu, Windows/UAC/sterowników oraz uprawnionego API i hybrydy. Bielik1.5B
   pozostaje wyłącznie pomocniczy; nie ma dowodu działania konta Luna/Haiku tej sesji.

Cloud bez GPU/mikrofonu nie zastępuje tych pomiarów. Nie uruchamiano mostówCLI,
subagentów ani harmonogramów, nie czytano sekretów lub cookies użytkownika.

Pozostają też bazowe ostrzeżenia cargo-deny i deprecacje narzędzi. Wcześniejszy
pełny Prettier wskazał 16 nietkniętych plików źródłowych, 2 wyniki generowane i
2 przypięte pliki vendor; nie wykonywano masowego formatowania poza zakresem.
Zmienione pliki i kontrole Clippy/typów miały własne zerowe wyniki, co nie oznacza
zera wszystkich ostrzeżeń w całym projekcie. Szczegóły w zachowanym raporcie CI191.
Bielik11B na CPU był NOT_RUN_CPU_BUDGET przy limicie RAM8192MB; to nie pomiar
dopasowania modelu i kontekstu do RTX2060 12GB. Realne API/hybryda nadal bez odbioru.
