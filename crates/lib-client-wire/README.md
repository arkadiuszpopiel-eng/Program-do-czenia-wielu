# lib-client-wire

Czysta biblioteka formatu JSON klient–host, wspólna dla przyszłej bramy i klientów
(ADR0019). Nie jest modułem wykonującym polecenia ani gotowym transportem telefonu.

`CommandFrame { v, id, cmd, args }` ma schemat Rust→JSON→TS przez istniejący generator
app-api. `CommandDecoder` wymaga jawnego niezerowego limitu bajtów i wersji; nie narzuca
budżetu hosta. Sprawdza limit przed JSON, ścisły format, wersję, dokładny zakres liczb
JavaScript oraz rosnące ID. Zamkniętego dekodera nie można otworzyć ponownie. Błędy i
Debug nie zawierają nazw komend ani argumentów. Limit rekurencji serde_json pozostaje.

`ProtocolVersions` wybiera najwyższą wspólną wersję spośród bieżącej i poprzedniej,
z minimum hosta. Wersje i limit są konfiguracją wywołującego, bez nowych domyślnych progów.

Przyszłe wpięcie musi zachować kolejność:

1. Zweryfikować kanał E2E, sparowane urządzenie i aktualną sesję po stronie hosta.
2. Utworzyć osobny dekoder dla kanału; odwołanie urządzenia zamyka kanał i dekoder.
3. Po zdekodowaniu sprawdzić rejestr komend, ACL i niezmienioną tabelę local_only,
   następnie typed DTO i Broker. Pole `approved` w args pozostaje niezaufanymi danymi.
4. Nieznana komenda nie może trafić do wykonawcy. Parser zachowuje nazwę do tej kontroli;
   samo otrzymanie CommandFrame nie jest dowodem autoryzacji.
5. Zużytego ID nie można powtórzyć po odmowie ACL/Brokera. Kontrola nonce E2E i związania
   sesji pozostaje obowiązkiem kanału; lokalny licznik nie zastępuje kryptografii.

Brak sieci, kluczy, listenera, parowania, ACL, dispatchera, dowodów zgody, wznowienia
zdarzeń lub logów użytkownika. Istniejący EventLog app-api pozostaje źródłem wznowienia.
Biblioteka jest wyłącznie formatem; nie włącza dostępu zdalnego i nie zmienia polityk.

Testy: ścisła ramka/UTF8, granice bajtów i identyfikatorów, powtórzenia, zamknięcie,
negocjacja, nieznane pola i podrabiane deklaracje autorytetu, głęboki JSON, redakcja
Debug oraz właściwości licznika i losowych bajtów. Wejścia wyłącznie syntetyczne.
