# #69 — pakiet decyzji dla rzeczywistego sterowania telefonem

**Stan: projekt zatwierdzony przez właściciela12.11.2026, bez kodu transportu i bez odbioru telefonu.**
Pełna zgoda na dostępne operacje komputera została udzielona; poniższy zakres dotyczy
osobnych decyzji architektury i dowodów zgód urządzenia. [Podpis i zakres](2026-11-12-owner-decisions.md).

## Warunek zapisany w repozytorium

[ADR0021](../ADR/0021-dostep-zdalny-przekaznik-e2e-i-parowanie.md):
„Kod powstaje dopiero po podpisie ADR 0019–0023 (zadanie D.0).”
Wzorzec Noise dla klienta natywnego zatwierdzono razem z projektem; bibliotekę, klienta WWW
i konkretną implementację nadal trzeba zweryfikować.
[ADR0022](../ADR/0022-zgody-zdalne-dowod-urzadzenia-sparowanego.md) wymaga osobnego
przeglądu zmiany Brokera. Właściciel12.11 zatwierdził projekt jako podpis tych ADR; konkretna implementacja Brokera nadal osobno do przeglądu.

## Rozstrzygnięcia projektu zatwierdzone12.11.2026

| Zakres | Konkretna propozycja | Co pozostaje do sprawdzenia |
|---|---|---|
| ADR0019 | Jedne wersjonowane ramki i generowane DTO, ACL na hoście, `local_only` odrzucane lub obsługiwane przez jawny odpowiednik | Kompletna tabela aktualnych komend, redakcja ścieżek, wznowienie z epoch/seq/resync |
| ADR0020 | Najpierw brama wewnątrz uruchomionej aplikacji; wydzielenie alfa-host dopiero po jej odbiorze | Zamykanie/aktualizacja/kill-switch, późniejsza chroniona łatka launchera i watchdoga |
| ADR0021 | Wychodzący WSS obu stron, własny przekaźnik właściciela; propozycja `Noise_XX_25519_ChaChaPoly_SHA256` dla klienta natywnego | Dobór biblioteki i zgodnego klienta WWW, test vectors, audyt implementacji i przypiętych kluczy; sam XX nie daje zaufania bez parowania |
| Parowanie | QR2min wiążący host/relay/publiczny klucz i jednorazowy sekret; jednorazowe zużycie; zgodny kod kontrolny; pierwsze urządzenie fizycznie w Broker-UI | Binding sekretu z transkryptem Noise i tożsamościami; wygaśnięcie, próby zgadywania, podmiana QR, brak sekretu w URL/logu |
| ADR0022 | Wszystkie dostępne działania także wysokiego ryzyka; zakres urządzenia ogranicza host, a konkretną zgodę niezależnie sprawdza Broker | Dokładna łatka kontraktu i dowodu, odwołanie, podpis żądania, brak samozatwierdzenia przez agentkę/bramę |
| WebAuthn | Osobny klucz zgód od klucza kanału; challenge wiąże wersję, urządzenie, requestHash, nonce i termin | Broker weryfikuje podpis, rpIdHash, origin, challenge, UP/UV i ważność; nie ufa przesłanemu `user_verified=true`; problem klonowanego/zerowego signCount rozwiązuje nonce |
| ADR0023 | Panel WWW bez hosta: wariantA, bez klucza API w przeglądarce; Android może dostać klucz wyłącznie po odrębnej zgodzie i przeglądzie magazynu | Windows Credential Manager → E2E → magazyn Androida wymaga zatwierdzonej zmiany polityki; odwołanie nie gwarantuje usunięcia z nieosiągalnego telefonu |

Wybór własnego przekaźnika jest zatwierdzony; nie jest to uruchomiona usługa lub zgoda na koszty.
Nie ma automatycznej instalacji nowych zależności, publikacji panelu ani odczytu kluczy.
Nie zatwierdzono tu nowego progu czasu odwołania, wielkości ramki lub limitu zgód.

## Przyrosty po przeglądzie

1. Kontrakty/fake gateway–relay–pairing: jawna tożsamość klienta, sesja, korelacja żądania,
   błędy, anulowanie i budżety. Żaden schemat nie przyjmuje dowolnego kodu z interfejsu.
2. Kanał i parowanie: przypięte tożsamości, klucze sesyjne, liczniki i nowe klucze po
   ponownym połączeniu; zakres urządzenia i dziennik odwołania trzymane na hoście.
3. Zdalny AlfaClient: ten sam protokół, kontrola `local_only` po stronie hosta,
   wykrywanie luk seq/zmiany epoch, jawny resync i ochrona przed podwójną akcją po zerwaniu.
4. Panel dotykowy: rozmowa, głos, zadania, pliki, obraz hosta, obsada, modele, zgody;
   stany łączenia/online/utrata/odwołane/host-offline. Stop ma najwyższy priorytet.
5. Chroniona łatka zgód PairedDevice do osobnego przeglądu, nie ogólna zgoda w interfejsie.
6. Odbiór rzeczywistego telefonu przed włączeniem: także zerwanie Wi-Fi/LTE, tło,
   obrót, powiększenie tekstu, czytnik ekranu i zużycie baterii.

## Macierz obowiązkowych prób

| Próba | Wymagany wynik |
|---|---|
| Przekaźnik zapisuje cały ruch | Znany tekst rozmowy/zgody i klucze nie występują w zapisie; IP/czas/rozmiar pozostają jawnym ograniczeniem |
| Podmiana hosta/klucza/QR | Parowanie odrzucone, brak automatycznego zaufania i brak wykonywania komend |
| Powtórzona/przestawiona/ucięta/za duża ramka | Odrzucona, bez akcji; kontrolowany limit i zerwanie |
| Zgoda na inny requestHash/nonce/origin/rpId lub bez UV | Broker odmawia, zero samodzielnych zgód agentki |
| Odwołanie aktywnego urządzenia | Kanał kończy się, żądania w kolejce nie otrzymują nowych uprawnień; mierzony czas względem zatwierdzonego progu |
| Utrata połączenia po rozpoczętej operacji | Brak automatycznego odtworzenia mutacji; odczyt rzeczywistego wyniku po resync |
| Brak uprawnienia lub `local_only` | Odmowa po stronie hosta także dla ręcznie zmienionego klienta |
| Host wyłączony | Jawny brak narzędzi komputera; panelWWW nie twierdzi, że wykonał operację lokalną |
| Stop/kill-switch, zawieszony klient i zalanie ramek | Zakończenie własnych zadań, ograniczone kolejki, zgody bez obejścia polityk |

Obecny panel to podgląd: transport gateway/relay/pairing nadal nie istnieje. Ta macierz
jest planem testów, **nie wynikiem testów**. Nie ma zgody na zmianę zamrożonych evals.
Przed scaleniem potrzebny świeży niezależny przegląd, zielone CI i właściwy odbiór sprzętu.
