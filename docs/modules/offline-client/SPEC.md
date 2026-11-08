# offline-client — SPEC (szkic v0, planowany — plan v6)

## Cel
Biblioteka klienta dla trybu „bez komputera”: gdy host jest nieosiągalny, prowadzi samą rozmowę bezpośrednio
z dostawcą API (ten sam dostawca i model co na komputerze), zapisuje tury lokalnie w postaci zaszyfrowanej
i po powrocie połączenia oddaje je hostowi do dopisania jako nową gałąź. Moduł **nie** wykonuje narzędzi,
nie ma pamięci, plików ani modelu lokalnego i nie jest synchronizacją dwukierunkową. Stan: propozycja.

## Fala i priorytet
Plan v6, **etap F** (zadanie F.6), po podpisie ADR 0023. Priorytet: do ustalenia przez właściciela.
v0: Android (klucz w Android Keystore). Panel WWW (E.6) — wstrzymany do rozstrzygnięcia ADR 0023.

## Kontrakt (szkic Rust)
```rust
// szkic — źródłem prawdy będzie crate offline-client-contract
pub struct OfflineProfile { pub provider: ProviderId, pub model: ModelId, pub synced_at: Timestamp }
pub struct OfflineTurn { pub session: SessionId, pub parent: Option<TurnId>, pub role: Role,
                         pub content: String, pub created_at: Timestamp, pub usage: Option<TokenUsage> }
pub struct MergeBatch { pub session: SessionId, pub base_turn: Option<TurnId>, pub turns: Vec<OfflineTurn> }
pub trait OfflineChat {
    fn available(&self) -> bool;                                   // jest zgoda, klucz i profil
    fn remember_profile(&self, profile: OfflineProfile);           // przy każdym połączeniu z hostem
    fn send(&self, session: SessionId, text: String) -> Result<TurnStream, OfflineError>;
    fn pending(&self) -> Vec<MergeBatch>;                          // kolejka do scalenia
    fn mark_merged(&self, session: SessionId, up_to: Timestamp) -> Result<(), OfflineError>;
    fn wipe(&self) -> Result<(), OfflineError>;                    // odwołanie / wylogowanie
}
```
Po stronie hosta polecenie przyjmujące `MergeBatch` (nazwa do ustalenia w `app-api`).
Zdarzenia: `offline-client.entered`, `.left`, `.merge.done`, `.wiped` (bez treści rozmów).

## Zależności
`providers-api-contract` (ten sam adapter dostawcy co na hoście), `sessions-contract` (kształt tury i gałęzi),
`cost-meter-contract` (zużycie do doliczenia po scaleniu), `platform-contract` (`SecretStore` / `KeyVault` —
na Androidzie z `platform-android`).

## Niezmienniki
- Bez hosta nie ma narzędzi ani działań: klient offline nie ma drogi do Brokera i niczego nie wykonuje.
- Klucz API trafia na urządzenie tylko po osobnej zgodzie na komputerze, kanałem E2E, do bezpiecznego magazynu;
  nigdy do plików, logów ani pamięci zwykłej aplikacji w postaci trwałej.
- Scalanie tylko dopisuje: tury trafiają do historii jako **nowa gałąź** od ostatniej wspólnej tury; żadna
  istniejąca tura nie jest zmieniana (ADR 0006).
- Ponowne wysłanie tej samej paczki nie tworzy duplikatów (scalanie idempotentne).
- Tury z urządzenia są oznaczone „prowadzone bez komputera”.
- Rozmowy na urządzeniu są zaszyfrowane kluczem z magazynu; `wipe` usuwa klucz API i rozmowy (R11).
- Interfejs zawsze pokazuje stan „Bez komputera — rozmowa przez model w chmurze”.

## Zdolności / uprawnienia
Na urządzeniu: wyjście sieciowe tylko do hosta API dostawcy zapamiętanego z profilu. Zgoda per urządzenie
(`DeviceScope.offline_chat`) wydawana na komputerze. Brak tokenów narzędzi.

## Izolacja
W procesie aplikacji klienckiej (Android: część Rust aplikacji Tauri mobile), `lazy` — aktywna tylko, gdy host
jest nieosiągalny albo czeka kolejka do scalenia.

## Budżet zasobów
Miejsce na urządzeniu zajmowane przez niezscalone rozmowy, zużycie baterii i danych — do pomiaru; progi
(w tym limit rozmiaru kolejki) ustala właściciel.

## Konfiguracja (klucze TOML)
Po stronie hosta (szkic): `[remote.offline] allowed = false` — tryb domyślnie wyłączony do podpisu ADR 0023.
Po stronie urządzenia: zapamiętany `OfflineProfile`; bez pliku TOML.

## Wkład do UI
Baner stanu „Bez komputera”, wyszarzone funkcje spoza mapy zdolności, znacznik przy turach prowadzonych bez
komputera, komunikat o scaleniu po powrocie. Na komputerze: zgoda „Pozwól temu urządzeniu rozmawiać bez
komputera” i przypomnienie o wymianie klucza przy odwołaniu urządzenia.

## Testy akceptacyjne
- `ACC-V6-offline-client-01` — po wyłączeniu komputera rozmowa działa przez model chmurowy; po powrocie pojawia
  się w historii na komputerze jako nowa gałąź.
- `ACC-V6-offline-client-02` — scalanie nie zmienia żadnej istniejącej tury (porównanie skrótów historii przed i po).
- `ACC-V6-offline-client-03` — dwukrotne scalenie tej samej paczki nie tworzy duplikatów.
- `ACC-V6-offline-client-04` — urządzenie bez zgody na tryb bez komputera nie otrzymuje klucza API (liczbę prób ustala właściciel).
- `ACC-V6-offline-client-05` — po odwołaniu osiągalnego urządzenia klucz i rozmowy znikają poniżej progu (próg ustala właściciel).
- `ACC-V6-offline-client-06` — koszt rozmów bez komputera jest po scaleniu w liczniku kosztów.
- `ACC-V6-offline-client-07` — w trybie bez komputera żadne narzędzie nie jest wywoływane (test własności).

## Fake
`offline-client-fake`: dostawca API z nagrania (record/replay, jak fake LLM), magazyn kluczy w pamięci,
wirtualny zegar, sterowana osiągalność hosta (zniknięcie i powrót w trakcie tury), host testowy przyjmujący paczki.

## Otwarte pytania
- ADR 0023 — klucz API w przeglądarce (opcje A/B); do rozstrzygnięcia moduł działa tylko na Androidzie.
- ADR 0023 — limit wydatków na urządzeniu bez hosta (limit miesięczny żyje na hoście).
- ADR 0006 ma status „Tymczasowy” — zmiana modelu gałęzi wymaga przeglądu tego SPEC.
- Czy instrukcje systemowe persony i fragment wcześniejszej rozmowy są zapamiętywane na urządzeniu (prywatność
  a ciągłość rozmowy) — plan mówi tylko o dostawcy i modelu; do ustalenia.
- Język biblioteki: Rust (Android przez Tauri mobile) wystarcza dla v0; wariant dla przeglądarki — po decyzji ADR 0023.
- Co się dzieje, gdy na hoście zmieniono dostawcę lub klucz, a urządzenie ma stary profil — do ustalenia.
