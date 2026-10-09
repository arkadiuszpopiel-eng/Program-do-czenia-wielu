# platform-android — SPEC (szkic v0, planowany — plan v6)

## Cel
Implementacje portów dla aplikacji Android (Android 15+, Tauri 2 mobile): bezpieczny magazyn kluczy,
biometria przy podpisie, powiadomienia. Telefon jest **pilotem** hosta, nie hostem: moduł **nie** dostarcza
portów plików, procesów, powłoki, UIA ani modelu lokalnego i nie zawiera Brokera. Kod specyficzny dla Androida
żyje tylko tutaj. Stan: propozycja, crate'y nie istnieją.

## Fala i priorytet
Plan v6, **etap F** (zadanie F.2; powiązane F.1, F.3, F.5), po etapie D i E.1 oraz po podpisie ADR 0017,
0021–0023. Priorytet: do ustalenia przez właściciela. v0: magazyn kluczy, biometria, powiadomienia, skan QR.
v1: głos — mikrofon i głośnik telefonu z przetwarzaniem na hoście, usługa pierwszoplanowa (F.5).

## Kontrakt (szkic Rust)
```rust
// szkic — porty zostają w kontraktach; tu tylko implementacje i brakujące porty urządzenia
pub trait DeviceKeyStore {                       // klucz urządzenia: nieeksportowalny, użycie = biometria
    fn create_device_key(&self) -> Result<PublicKey, KeyError>;
    fn sign(&self, request_hash: [u8; 32], prompt: BiometricPrompt) -> Result<UserVerifiedSignature, KeyError>;
    fn delete_all(&self) -> Result<(), KeyError>;
}
pub trait NotifyPort { fn show(&self, n: Notification) -> Result<(), PlatformError>; }  // bez treści rozmów
pub trait QrScanPort { fn scan(&self) -> Result<String, PlatformError>; }
// SecretStore (istniejący kontrakt) → Android Keystore: klucz API dostawcy dla trybu bez komputera
pub fn platform_android() -> PlatformPorts;      // fs/exec/uia/audio lokalne: brak → Unsupported
```
Zdarzenia: `platform.notification.opened`, `platform.biometric.failed` (bez danych biometrycznych).

## Zależności
`platform-contract`, kontrakt z `SecretStore` / `KeyVault`, `platform-select` (gałąź `target_os = "android"`).
Zewnętrzne: `jni` i wtyczki Kotlin tam, gdzie brak gotowych wtyczek Tauri — dozwolone tylko w crate'ach
`platform-android-*` (reguła w `deny.toml`); wersje i opis w `docs/vendor/` przed kodem.

## Niezmienniki
- Klucz urządzenia powstaje w Android Keystore i nigdy go nie opuszcza; każde użycie do podpisu zgody wymaga
  biometrii (albo PIN-u urządzenia) — sam odblokowany ekran nie wystarcza.
- `user_verified` w dowodzie zgody jest prawdziwe tylko wtedy, gdy system potwierdził weryfikację przy tym podpisie.
- Klucz API dostawcy jest przechowywany wyłącznie w magazynie kluczy; nieczytelny dla innych aplikacji (R4).
- Rozmowy na urządzeniu szyfrowane kluczem z magazynu; odwołanie i wylogowanie kasują klucze i dane (R11).
- Powiadomienia nie zawierają treści rozmów ani szczegółów żądań — tylko sygnał „obudź i pobierz”.
- Poza `platform-android-*` brak `jni` i API Androida w workspace (test grafu zależności).
- Porty nieobecne na telefonie zwracają `Unsupported`; mapa zdolności nie zgłasza ich klientowi.

## Zdolności / uprawnienia
Uprawnienia systemu Android: aparat (skan QR), powiadomienia, biometria, sieć; mikrofon i usługa
pierwszoplanowa dopiero w v1. Tokenów Brokera brak — Broker działa na hoście.

## Izolacja
`inproc` w aplikacji Android (piaskownica aplikacji systemu), `lazy`. Kod Kotlin wywoływany przez wtyczki Tauri.

## Budżet zasobów
Rozmiar APK, RAM aplikacji, zużycie baterii w tle, czas od powiadomienia do pokazania prośby o zgodę — do
pomiaru na telefonie testowym właściciela; progi ustala właściciel.

## Konfiguracja (klucze TOML)
Brak plików TOML na urządzeniu. Ustawienia aplikacji (szkic): adres przekaźnika z parowania, powiadomienia
włączone / wyłączone, wymagana klasa biometrii. Zakres urządzenia ustawia host (`DeviceScope`).

## Wkład do UI
Ten sam interfejs Svelte w układzie od 360 px; dodatkowo natywne: skaner QR, systemowe okno biometrii przy
każdej zgodzie, powiadomienia o prośbach o zgodę, ekran „Bez komputera”.

## Testy akceptacyjne
- `ACC-V6-platform-android-01` — APK buduje się w CI (F.1) i uruchamia w emulatorze Androida 15.
- `ACC-V6-platform-android-02` — podpis zgody bez udanej biometrii nie powstaje (liczbę prób ustala właściciel).
- `ACC-V6-platform-android-03` — telefon sparowany aparatem poniżej progu (cel planu: mniej niż minuta; próg ustala właściciel).
- `ACC-V6-platform-android-04` — po odwołaniu urządzenia klucze i rozmowy są usunięte przy najbliższym połączeniu.
- `ACC-V6-platform-android-05` — treść powiadomienia nie zawiera znanego tekstu rozmowy ani żądania.
- `ACC-V6-platform-android-06` — pełna obsługa z telefonu: rozmowa, agentki, pliki hosta, zgody, STOP (F.4; na urządzeniu).
- `ACC-V6-platform-android-07` — zużycie baterii w tle poniżej progu (próg ustala właściciel).

## Fake
`platform-fake` rozszerzony o atrapy: magazyn kluczy w pamięci ze sterowanym wynikiem biometrii (udana,
nieudana, anulowana), rejestr powiadomień, skaner QR zwracający zadany tekst. Testy sprzętowe — telefon właściciela.

## Otwarte pytania
- ADR 0017 — zakres Androida jako pilota; ADR 0021 — rodzaje kluczy (kanał Noise a podpis zgód) dostępne
  w magazynie Androida — do sprawdzenia przed kodem.
- ADR 0022 — wymagana klasa biometrii i czy PIN urządzenia wystarcza jako weryfikacja użytkownika.
- ADR 0023 — przekazanie i kasowanie klucza API; zachowanie, gdy telefon nie odbierze polecenia skasowania.
- Powiadomienia push: usługa systemowa dostawcy czy własne połączenie w tle (prywatność, bateria) — do ustalenia.
- Dojrzałość Tauri 2 mobile na Androidzie jest w planie oznaczona jako niezweryfikowana — potrzebny spike przed F.2.
- Publikacja: APK do pobrania; sklep — decyzja właściciela (F.8).
