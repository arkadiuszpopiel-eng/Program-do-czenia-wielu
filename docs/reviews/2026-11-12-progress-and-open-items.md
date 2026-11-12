# Raport prac i otwartych spraw — 12 listopada 2026

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
