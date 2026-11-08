# device-pairing — SPEC (szkic v0, planowany — plan v6)

## Cel
Parowanie urządzeń z hostem kodem QR, trwała lista urządzeń (nazwa, klucz publiczny, zakres uprawnień)
i odwołanie urządzenia. Moduł **nie** prowadzi sesji klientów ani ruchu (`remote-gateway`), nie sprawdza
dowodów zgody (robi to Broker kluczem z tej listy) i nie przechowuje kluczy prywatnych urządzeń — te nigdy
nie opuszczają bezpiecznego magazynu urządzenia. Stan: propozycja, crate nie istnieje.

## Fala i priorytet
Plan v6, **etap D** (zadanie D.2), po podpisie ADR 0019–0023 (D.0). Priorytet: do ustalenia przez właściciela.
v0: parowanie pierwszego urządzenia przy komputerze, lista, odwołanie. v1: dodanie kolejnego urządzenia
z już sparowanego, zakres per urządzenie, zgoda „rozmowa bez komputera” (ADR 0023).

## Kontrakt (szkic Rust)
```rust
// szkic — źródłem prawdy będzie crate device-pairing-contract
pub struct DeviceId(pub Uuid);
pub struct PairingOffer {            // treść kodu QR, ważna 2 minuty
    pub relay_url: String, pub host_id: HostId, pub host_public_key: PublicKey, pub one_time_secret: Secret,
}
pub struct PairedDevice {
    pub id: DeviceId, pub name: String, pub kind: DeviceKind /* Android | Browser | Desktop */,
    pub public_key: PublicKey, pub paired_at: Timestamp, pub last_seen: Option<Timestamp>, pub scope: DeviceScope,
}
pub struct DeviceScope { pub commands: CommandAcl, pub may_approve_high_risk: bool, pub offline_chat: bool }
pub trait DevicePairing {
    fn begin(&self) -> Result<PairingOffer, PairingError>;
    fn check_code(&self, offer: &PairingOffer) -> Result<ShortCode, PairingError>;   // np. 6 cyfr
    fn confirm(&self, code: ShortCode, proof: ApprovalProof) -> Result<PairedDevice, PairingError>;
    fn list(&self) -> Vec<PairedDevice>;
    fn set_scope(&self, id: DeviceId, scope: DeviceScope, proof: ApprovalProof) -> Result<(), PairingError>;
    fn revoke(&self, id: DeviceId) -> Result<(), PairingError>;
    fn revoke_all(&self) -> Result<(), PairingError>;
}
```
Zdarzenia: `device-pairing.offer.created`, `.device.paired`, `.device.scope_changed`, `.device.revoked`.

## Zależności
`safety-broker-contract` (dowód zgody, Audyt), `platform-contract` (`SecretStore` / `KeyVault` na klucz hosta),
`core-bus-contract`, `core-config-contract`, `core-log-contract`. Zewnętrzne: generator kodu QR (do wyboru,
wpis w `docs/vendor/`).

## Niezmienniki
- **Pierwsze urządzenie** wchodzi tylko po potwierdzeniu kodu kontrolnego fizycznym kliknięciem w oknie Brokera
  na komputerze (`ApprovalProof::PhysicalInput`); tego kroku nie da się wykonać zdalnie.
- Sekret jednorazowy działa raz i wygasa po 2 minutach; oferta użyta albo przeterminowana jest odrzucana.
- Kolejne urządzenie dodane zdalnie wymaga dowodu `PairedDevice` ze świeżą biometrią i jest w Audycie
  zdarzeniem wysokiego ryzyka.
- Odwołanie jest natychmiastowe (klucz usunięty, kanał zerwany, późniejsze dowody odrzucane) i można je wykonać
  z każdego sparowanego urządzenia oraz z panelu WWW.
- Lista urządzeń jest poza zasięgiem agentek i Ulepszacza (jak inne dane Jądra); żadnego nasłuchu sieciowego.
- Klucz prywatny hosta nie trafia do plików konfiguracji, logów ani eksportu `.alfa`.

## Zdolności / uprawnienia
`secrets.read` / zapis klucza hosta w magazynie systemu. Parowanie, zakres i odwołanie — tylko właściciel.

## Izolacja
`inproc`, `lazy` (przy otwarciu Ustawień → Urządzenia albo starcie bramy); miejsce listy urządzeń — patrz pytania.

## Budżet zasobów
RAM i CPU pomijalne; czas parowania od skanu do gotowości — cel planu „mniej niż minuta”, próg ustala właściciel.

## Konfiguracja (klucze TOML)
Szkic: `[remote.pairing] offer_ttl = "2m"`, `short_code_digits = 6`, `max_devices = <limit>`,
`high_risk_default = true`. Lista urządzeń nie jest w TOML — żyje w magazynie chronionym (do ustalenia gdzie).

## Wkład do UI
Ustawienia → Urządzenia: „Dodaj urządzenie” (kod QR + kod kontrolny), lista urządzeń, **Odwołaj**, **Odwołaj
wszystkie**, zakres z przełącznikiem wysokiego ryzyka. Potwierdzenie kodu — w oknie Brokera, nie w WebView.

## Testy akceptacyjne
- `ACC-V6-device-pairing-01` — oferta po upływie ważności i oferta użyta drugi raz są odrzucane.
- `ACC-V6-device-pairing-02` — pierwsze urządzenie bez fizycznego potwierdzenia: 0 sukcesów (liczbę prób ustala właściciel).
- `ACC-V6-device-pairing-03` — podmieniony kod QR daje różne kody kontrolne po obu stronach (R5).
- `ACC-V6-device-pairing-04` — po odwołaniu dowód zgody jest odrzucany, a kanał zerwany poniżej progu (próg ustala właściciel).
- `ACC-V6-device-pairing-05` — odwołanie z innego sparowanego urządzenia działa.
- `ACC-V6-device-pairing-06` — czas parowania telefonu na sprzęcie poniżej progu (próg ustala właściciel).

## Fake
`device-pairing-fake`: lista urządzeń w pamięci, wirtualny zegar (wygasanie oferty), klucze testowe
deterministyczne, „urządzenie” skryptowane (zgodny / niezgodny kod kontrolny, odwołane w trakcie).

## Otwarte pytania
- ADR 0021 — które klucze urządzenia służą do kanału Noise, a które do podpisu zgód; w przeglądarce klucz
  dostępu WebAuthn nie nadaje się wprost do uzgadniania kanału.
- ADR 0022 — domyślna wartość przełącznika wysokiego ryzyka (rekomendacja sesji: włączony) i miejsce
  przechowywania listy urządzeń względem procesu Brokera.
- ADR 0023 — przebieg zgody „Pozwól temu urządzeniu rozmawiać bez komputera”.
- Przybliżona lokalizacja z adresu IP liczona lokalnie — sposób do ustalenia; zmiana adresu przekaźnika
  a istniejące parowania — do ustalenia w SPEC v1.
