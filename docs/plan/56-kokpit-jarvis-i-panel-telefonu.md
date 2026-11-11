# 56 — kokpit Jarvis, kafelki telefonu i panel zdalny

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
