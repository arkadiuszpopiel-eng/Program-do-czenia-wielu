# web-panel — SPEC (szkic v0, planowany — plan v6)

## Cel
Panel WWW: ten sam interfejs Svelte co w oknie Tauri, zbudowany jako aplikacja PWA i połączony z hostem przez
przekaźnik i kanał end-to-end. Pilot do Alfy z dowolnej przeglądarki, także na cudzym komputerze. Moduł
**nie** zawiera logiki hosta, nie ma własnych typów domenowych (typy generowane z Rust) i w v0 **nie** działa
bez komputera (ADR 0023). Stan: propozycja, kod nie istnieje.

## Fala i priorytet
Plan v6, **etap E** (zadania E.1–E.7), po etapie D i po podpisie ADR 0019–0023. Priorytet: do ustalenia przez
właściciela. v0: układ od 360 px (E.1), budowa PWA (E.2), parowanie i logowanie kluczem dostępu (E.3), zgody
z potwierdzeniem kluczem dostępu (E.4), hosting i CSP (E.7). v1: powiadomienia push bez treści (E.5);
tryb bez komputera (E.6) — tylko jeśli właściciel wybierze opcję B w ADR 0023.

## Kontrakt (szkic Rust)
Moduł jest klientem TypeScript — nie ma crate'a Rust. Kontraktem jest istniejący interfejs `AlfaClient`
(`apps/desktop/ui/src/lib/api/client.ts`) i protokół z ADR 0019; typy pochodzą z generatora (ADR 0013).
```ts
// szkic — trzecia implementacja AlfaClient obok TauriAlfaClient i FakeAlfaClient (zadanie D.6)
class RemoteAlfaClient implements AlfaClient {
  constructor(channel: E2eChannel, device: DeviceKey) {}
  invoke<C extends CommandName>(cmd: C, args: ArgsOf<C>): Promise<ResultOf<C>>; // ramka { v, id, cmd, args }
  subscribe(onEvents: (seq: number, events: AlfaEvent[]) => void): Unsubscribe; // po przerwie events_resume
  capabilities(): Promise<HostCapabilities>;                                   // co pokazać, co ukryć
}
```
Zdarzenia na magistrali hosta: brak własnych (panel jest odbiorcą `AlfaEvent`).

## Zależności
Pakiet interfejsu (`apps/desktop/ui`, `packages/ui-kit`), typy generowane z `app-api`. Po stronie hosta:
`remote-gateway`, `device-pairing`. Zewnętrzne: WebAuthn i WebCrypto przeglądarki, implementacja wzorca Noise
dla przeglądarki (wybór do ustalenia — wpis w `docs/vendor/` przed kodem).

## Niezmienniki
- Panel nie ma skrótu do narzędzi: każde działanie idzie przez tego samego hosta i tego samego Brokera.
- Panel pokazuje tylko funkcje z mapy zdolności hosta i tylko polecenia z ACL swojej sesji; ukrycie przycisku
  nie jest zabezpieczeniem — odmowa następuje po stronie hosta.
- Zgoda wymaga potwierdzenia kluczem dostępu z `userVerification: required` przy **każdym** podpisie.
- Klucz dostępu jest związany z domeną panelu — na innej domenie nie zadziała (R6).
- Treść z modelu przychodzi jako HTML wyrenderowany w Rust na hoście (ADR 0009); panel go tylko wstawia.
- Żadnych fontów webowych ani `backdrop-filter` (reguły twarde projektu); ścisła CSP i integralność zasobów (R7).
- Klucz API dostawcy nie jest przechowywany w przeglądarce, dopóki ADR 0023 tego nie rozstrzygnie.
- Powiadomienia push nie niosą treści („obudź i pobierz”).

## Zdolności / uprawnienia
Brak własnych tokenów Brokera. Uprawnienia panelu to zakres sparowanego urządzenia (`DeviceScope`) nadany
na hoście; domyślnie pełny (decyzja właściciela 3).

## Izolacja
Poza hostem — działa w przeglądarce użytkownika. Zasoby statyczne z hostingu panelu (stała domena albo własny
serwer właściciela). Service worker tylko do pamięci podręcznej zasobów i powiadomień.

## Budżet zasobów
Rozmiar paczki, czas do interakcji na telefonie przez sieć komórkową, opóźnienie pierwszego fragmentu odpowiedzi
przez przekaźnik — do pomiaru; progi ustala właściciel.

## Konfiguracja (klucze TOML)
Po stronie hosta brak nowych kluczy poza `[remote]` (`remote-gateway`). Po stronie panelu konfiguracja budowy
(szkic): domena panelu, domyślny adres przekaźnika. Preferencje interfejsu — w pamięci przeglądarki, bez sekretów.

## Wkład do UI
Cały interfejs Alfy w przeglądarce; dodatkowo: ekran parowania (krótki kod albo skan z telefonu), ekran
„komputer niedostępny”, karta zgody z pełnym opisem i przytrzymaniem przycisku dla wysokiego ryzyka,
przycisk STOP, lista urządzeń z odwołaniem.

## Testy akceptacyjne
- `ACC-V6-web-panel-01` — audyt dostępności (axe) zielony na 360 px i 1280 px.
- `ACC-V6-web-panel-02` — z przeglądarki działa wszystko, na co pozwala mapa zdolności hosta (scenariusze: sesja-recenzentka).
- `ACC-V6-web-panel-03` — zgoda bez świeżego potwierdzenia kluczem dostępu jest odrzucana przez Brokera.
- `ACC-V6-web-panel-04` — panel serwowany z innej domeny nie uwierzytelni się kluczem dostępu (R6).
- `ACC-V6-web-panel-05` — CSP blokuje skrypt spoza wydania; zasób o zmienionej sumie nie jest ładowany (R7).
- `ACC-V6-web-panel-06` — zerwanie sieci w trakcie odpowiedzi: po powrocie rozmowa jest kompletna.
- `ACC-V6-web-panel-07` — czas do interakcji poniżej progu (próg ustala właściciel).

## Fake
Istniejący `FakeAlfaClient` (interfejs działa w zwykłej przeglądarce bez hosta) + atrapa kanału zdalnego
(`remote-gateway-fake` z przekaźnikiem w pamięci) do testów wznowienia i odmów; wirtualny uwierzytelniacz
WebAuthn w testach Playwright.

## Otwarte pytania
- ADR 0023 — tryb bez komputera w przeglądarce (opcja A: brak; opcja B: klucz API zaszyfrowany kluczem pochodnym
  z klucza dostępu). Do rozstrzygnięcia tryb jest wyłączony.
- ADR 0022 — format dowodu zgody dla WebAuthn (asercja podpisuje własną strukturę, nie dowolny skrót).
- ADR 0021 — skąd klucz do kanału Noise w przeglądarce, skoro klucz dostępu służy do podpisu.
- Hosting panelu: stała domena czy wyłącznie własny serwer właściciela (R7) — decyzja właściciela.
- Polecenia `local_only` w panelu: wybór pliku z listy hosta, podgląd strumieniem — zakres w ADR 0019.
- Plan określa moduł jako „klienta”; układ katalogów (`apps/web`?) i relacja do trójki crate'ów — do ustalenia.
