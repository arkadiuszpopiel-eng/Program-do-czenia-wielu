# Panel telefonu — rozpoczęcie #69

Po zakończeniu lokalnych kontroliUI#68 (57/57Chromium,573/573Vitest,check/lint/docs0)
przygotowano podgląd panelu w osobnym worktree. To interfejs do przeglądu, nie wdrożony klient zdalny.

## Co faktycznie istnieje

- Podgląd otwierany przyciskiem telefonu w kokpicie, ładowany dopiero na żądanie.
- Stan „Brak połączenia · sterowanie zablokowane”. Parowanie i8poleceń są nieaktywne:
  rozmowa,głos,zadania,pliki,ekran,agentki,modele,uprawnienia. Brak handlerów wykonania.
- Brak fikcyjnegoQR, adresu hosta, sparowanych urządzeń albo potwierdzenia czynności.
- Dialog z tytułem/opisem, obsługąEscape, pułapką fokusu i powrotem do przycisku telefonu.
- Układ320/390/1280px,jasny/ciemny,PL/EN. Parametrlocale w przeglądarkowej atrapie umożliwia
  odtwarzalny testEN; nie ma wpływu na rzeczywisty klientTauri.

## Próby i wykryta usterka

RED0/1: podgląd nie istniał. Pierwsza szersza regresja55/57: oba błędy toaxe target-size
tytułu rozmowy w320px. Dodatkowy bezpośredni test belki odtworzyłRED0/1 bez dialogu.
Przy tej szerokości pomocniczy tytuł ustępuje kontrolkom; zmiana nazwy pozostaje przezF2/Sesje.
Nie wyłączono regułyaxe, nie zwiększono limitów i nie zmieniono polityk.
Końcowa regresja58/58Chromium: kokpit/panel, responsywność, dokowanie, klawiatura,
axe w320/390/1280px i2motywach,PL/EN,Escape/focus oraz0połączeńWebSocket.
Logi55/57, obaRED i końcowyGREEN zachowane; niczego nie zamaskowano ponowieniem.

## Ocena rzeczywistej kontroli z telefonu

**Nie ma jeszcze działającej drogi telefon→komputer.** W repozytorium remote-gateway,
remote-relay i device-pairing są specyfikacjami/propozycjami, bez gotowej implementacji.
app-broker/remote to komunikacja z procesemBrokera, nie telefon. Nie wykonano odbioru prawdziwego
telefonu, parowania, sieciInternet,WebAuthn albo zdalnego zatwierdzenia.

[ADR0021](../ADR/0021-dostep-zdalny-przekaznik-e2e-i-parowanie.md) ma status
„Proponowany — czeka na podpis właściciela” i wymaga: „Kod powstaje dopiero po podpisie ADR 0019–0023”.
WzorzecNoise pozostaje do ustalenia przez przegląd człowieka. PodglądUI nie zmienia tej decyzji,
nie otwiera produkcyjnego portu hosta, nie dodaje zależności, konta usług lub uprawnień.

## Konkretny backlog wdrożenia

1. Zamknąć przeglądADR0019–0023: wzorzecE2E,WebAuthn/klucze urządzeń i zakres zdalnych zgód.
2. Kontrakty/fake bramy klientów oraz protokół wersji/wznawiania; jawna lista komend iACL
   urządzenia. Brak ogólnego przekazywania dowolnegoIPC z przeglądarki.
3. Host i telefon wychodzą do przekaźnika, host bez nasłuchu. Wybór własnego przekaźnika lub
   dostawcy jest decyzją właściciela; nie wysłano danych do przypadkowej usługi.
4. Jednorazowe parowanie, kod kontrolny obu stron, pierwsza zgoda lokalnie wBrokerze;
   odwołanie urządzeń, wygasłe/powtórzone ramki i utrata sieci.
5. Broker sam weryfikuje dowód powiązany z dokładnym żądaniem i świeżą weryfikacją użytkownika;
   UI ani brama nie zatwierdzają same. Chronione zmiany wymagają konkretnego przeglądu człowieka.
6. Dopiero uwierzytelniony transport zasila panel i odblokowuje dozwolone funkcje. Host offline,
   cofnięcie urządzenia lub brak uprawnień blokują komendy, bez fikcyjnego sukcesu.
7. Odbiór na prawdziwym telefonie/Windows/Internecie, ponowne połączenie, cofanie i dziennik;
   niezależny przegląd oraz własne zieloneCI przed merge. Tryb bez hosta i cloud wymagają osobnego
   zatwierdzonego przechowywania kluczy i kont, nie tokenówCLI.

[Plan56](../plan/56-kokpit-jarvis-i-panel-telefonu.md). #41/#48,GPU12GB,semantyka person,
prawdziweAPI,mikrofon i dostawcaWWW pozostają otwarte. Podgląd nie oznacza jakościEnterprise.

Końcowe573/573Vitest,check0błędów/0ostrzeżeń,lint0,budżetpaczki0(574,1KB gzip łącznie),
103stronyHTML/odnośniki0. Własny pełnyCI i niezależny przegląd nowego heada pozostają wymagane.
[Zrzut bieżącego podglądu](artifacts/phone-panel-preview.png) — fake/headlessLinuxChromium,390px.
