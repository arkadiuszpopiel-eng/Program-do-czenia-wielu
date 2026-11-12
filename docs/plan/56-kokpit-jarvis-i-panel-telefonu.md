# 56 — kokpit Jarvis, kafelki telefonu i panel zdalny

**Decyzje właściciela12.11:** zatwierdzony projekt telefonu jako podpis ADR0019–0023
(D.0), własnyWSS/E2E, pierwsze parowanie lokalne, osobne dowody zgód, WWW bez kluczaAPI.
Pierwsza ścieżka #78: wyszukiwanie w przeglądarce bez kluczaAPI. Nie oznacza to
wdrożonego transportu/embeddingu; konkretne poprawki Brokera nadal osobno do przeglądu.
[Zapis zgody i zakres](../reviews/2026-11-12-owner-decisions.md).

**Przyrost #78 po wyborze właściciela:** `browser_search` bezAPI, jawny silnik
DuckDuckGo/Google/Bing, ta sama zgoda Brokera i BrowserPort. Potwierdza tylko HTTPS/host
strony, wymaga browser_read i oceny źródeł. Lokalne55/0/3 +1kontrola kataloguCore,
pełnyClippy0bezostrzeżeń; nie jest to żyweWWW ani osadzonyWebView2.
[Zakres i dowody](../reviews/2026-11-12-browser-search-no-api.md). Karta #78 nadal otwarta.

**Najnowsze CI12.11, kod372bfcd5:** CI191 **10/10**; RustLinux3631/0/28,
Windows3653/0/49; Vitest593, Chromium **963/963** bez pominięć/flaky.
Windows115 **3/4**, WebView263/0/5. Nowa macierz Core8B zatrzymała się na pierwszym
przypadku: FirstToken60s,0 odpowiedzi,29NOT_RUN, semantykaNOT_EVALUATED.
#58 pozostaje otwarte; nie zmieniono limitów ani nie ukryto czerwonego testu.
[Raport i dowody](../reviews/2026-11-12-execution-records-personas.md).
Poniższe wcześniejsze wyniki i blokady podpisu zachowują historię; aktualna decyzja D.0 jest powyżej.

**Korekta po CI190:**45df ma8/10zadań (UI960/1; ten sam błąd bramki zbiorczej),
RustLinux3630/0/27,Windows3652/0/48; Windows1144/4,WebView263/0/5.
Kodf03: odtworzone przepełnienie przycisku i ściśnięty tytuł, dwaRED0/2→48/48;
pełny nowy zestaw963 w przygotowaniu. Testye95: natywny Core/LocalProvider,
macierz30 przygotowana do osobnej oceny odpowiedzi, jeszcze niewykonana.
[Raport i ograniczenia](../reviews/2026-11-12-execution-records-personas.md).

**12.11 — fakty czynności i własne persony:** kod UI `894798a7`, backend `c7664edc`,
test formatu `c2c461e7`. Naprawiono brak danych wybranej gałęzi w kontekście kolejnej
tury, awarie kart/kokpitu dla własnych person oraz przypisanie roli z Kreatora.
Pełny Rust na c502: **3628/0/27**, 71/71 etapów i pełny spis celów. Core/API na c766:
15/0/0; kodek: 1 integracja, 30 przypadków/60 ciał, dostawca jako atrapa. Vitest na32a:
593/593; pełne UI na894: **961/961 po powtórzeniu całej drugiej części**. Pierwotna
porażka345/1/615 zachowana, przyczyna opóźnienia niepotwierdzona; bez zmiany limitów.
Nowa próba prawdziwego8B: dwa timeouty60s, cztery niewykonane, **NOT_EVALUATED**.
CI189/Windows113 dla cd55 nadal queued; kandydat ma osobną gałąź, bez własnego CI.
Karty22/41/48/58/68/69/77/78 pozostają otwarte. [Raport i surowe dane](../reviews/2026-11-12-execution-records-personas.md).
Linux/Chromium/fake nie jest odbiorem Windows/WebView2, telefonu, API, RTX12GB lub mikrofonu.

**11.11 — rozszerzenie właściciela:** mapa świata i systemy Jarvisa. Kod `2f3cf3e3`
dodaje opcjonalny Atlas: mapa offline28kB, odczyt stanów aplikacji i skróty do ustawień,
diagnostyki oraz agentek. Dwie regresje CSS naprawione:32→44px kontrolek telefonu
i ukrycie dodatkowego przycisku na małym ekranie. Końcowe41/41Chromium,573/573Vitest,
typy0/0,build/lint/format/generator0. [Raport i zrzuty](../reviews/2026-11-11-jarvis-atlas.md).
To fake/headlessLinux; fizyczny telefon i nowy pełny CI są odrębnymi kontrolami.

[#69 — konkretny pakiet decyzji](../reviews/2026-11-11-remote-design-review.md) obejmuje
bramę, WSS/E2E, parowanie i dowód zgód. Złożono pytanie właścicielowi o podpis
ADR0019–0023; do odpowiedzi nie zaczynamy zależnego kodu transportu. Konkretna zmiana
Brokera nadal wymaga osobnego przeglądu. Nie jest to wdrożone sterowanie telefonem.
[#78 — WebView2 i dodatki](../ADR/0031-wbudowana-przegladarka-webview2.md) ma nowy projekt;
wybór konta produkcyjnej wyszukiwarki pozostaje odrębny od przeglądarki.
[#58 — zachowane porażki nowego Cloud](../reviews/2026-11-11-ledger-diagnosis.md).

Nowe zlecenie właściciela10.11.2026: sprawdzić błędy i zgodność założeń, dopracować opcjonalny
filmowy Jarvis i kafelki mobilne, ocenić kontrolę przez telefon, następnie rozpocząć panel.
Poprzednie nocne terminy są zamknięte; automatyzacje pozostają wyłączone.

## Baza i kolejność

1. Modele13c8c88d: CI17910/10 orazWindows1014/4 sukces;Linux3567/0/26,
   Windows3593/0/47. To dowód kodu poprzednika, nie odbiór nowegoUI lub sprzętu.
2. [#68](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/68):
   audyt zrzutów desktop/390px/320px, opcjonalny kokpit, cele dotykowe, istniejąca nawigacja.
   RED0/2→GREEN2/2, końcowa regresja57/57; motywy/style/axe/niski ekran.
3. [#69](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/69):
   po kontroliUI przygotowano responsywny podgląd panelu telefonu, jawnie bez połączenia i bez
   aktywnych zdalnych poleceń. Nie udawać parowania lub wykonania czynności.
4. Przed wdrożeniem transportu: podpisy i przegląd zdalnego dostępu, kontrakty, parowanie,
   przekaźnikE2E, odwoływanie urządzeń, wznawianie sesji i autoryzacja każdej komendy.

## KryteriaUI

- Kokpit można wyłączyć; wybór nie zmienia uprawnień ani interakcji agentek.
- Rzeczywiste dane bieżącej sesji/obsady; dekoracyjny radar nie jest wykresem pomiarów.
- Jasny/ciemny motyw, istniejące style,PL/EN,klawiatura,focus i ograniczenie animacji.
- Kafelki min44px, 320px/390px/pion/poziom; composer pozostaje dostępny.
- Brak nowych zależności/fontów webowych/backdrop-filter. KontrolkiWindows nie znikają wTauri.
- Fake/headlessChromium nie jest fizycznym telefonem, WebView2, mikrofonem lub rzeczywistymAPI.

## Fakty o zdalnym dostępie

remote-gateway,remote-relay,device-pairing są propozycjami w dokumentacji, bez gotowych crate'ów
i bez klienta telefonu. app-broker/remote to komunikacja zBrokerem, nie dostęp telefonu.
[ADR0021](../ADR/0021-dostep-zdalny-przekaznik-e2e-i-parowanie.md) ma status
„Proponowany — czeka na podpis właściciela” i mówi: „Kod powstaje dopiero po podpisie ADR 0019–0023”.
WzorzecNoise i zasadyWebAuthn wymagają osobnego przeglądu człowieka. Nie zmieniamy tych decyzji.

Przekaźnik pozostaje poza komputerem właściciela; host nie otwiera portu. Pierwsze urządzenie
wymaga lokalnego potwierdzeniaBrokera. PodglądUI nie jest zgodą na zmianę polityk lub Brokera.
Do odbioru: rzeczywisty telefon, para/odwołanie/powtórzona ramka/utrata sieci, host offline,
zablokowane polecenia i potwierdzenia z ważnego urządzenia. Transport nie jest jeszcze wdrożony.

#41/#48, jakość wszystkich person na8–12B,RTX2060 12GB,prawdziweAPI/hybryda,mikrofon
oraz wybór dostawcyWWW pozostają otwarte. Zielone testyUI nie zamykają tych problemów.

[Podgląd panelu i backlog transportu](../reviews/2026-11-10-phone-panel.md).
