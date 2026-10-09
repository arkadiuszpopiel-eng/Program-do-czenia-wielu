# camera-hub — SPEC (szkic v0, planowany — plan v6)

## Cel

„System kamer” Alfy: jeden rejestr wszystkich kamer (USB, IP po kablu, Wi-Fi), kilka strumieni naraz, klatka na
żądanie i podgląd w niskiej rozdzielczości, stan każdej kamery. Moduł **nie** rozpoznaje osób, **nie** opisuje
obrazu i **nie** zapisuje nagrań — dostarcza klatki modułom `vision-describe` i `person-recognition`.
Stan (2026-10-09): istnieje podzbiór v0 — kamery USB przez `CameraPort`, przełącznik główny i per kamera,
klatka na żądanie, usypianie, `stop_all`, komendy interfejsu `cameras_*`; źródłem prawdy kontraktu jest crate
`camera-hub-contract` (szkic poniżej opisuje cel v1: kamery sieciowe, `add`/`remove`, podgląd strumieniowy,
zdarzenia `camera.*` na magistrali, zdolność `camera.view` — tego jeszcze nie ma). Przebiegu na fizycznej
kamerze jeszcze nie było. Opis całości: [`../../plan/12-kamery-i-rozpoznawanie-osob.md`](../../plan/12-kamery-i-rozpoznawanie-osob.md).

## Fala i priorytet

Plan v6, **grupa C3** (zadania C3.1–C3.5). v0: kamery USB, kilka naraz, usypianie, podgląd, strona „Kamery”,
wskaźnik w belce. v1: kamery sieciowe (RTSP, wyszukiwanie ONVIF), jakość połączenia, ponowne łączenie.
Nie wymaga ADR 0025 (bez biometrii).

## Kontrakt (szkic Rust)

```rust
// szkic — nazwy do ustalenia w SPEC v1
pub struct CameraId(pub String);
pub enum CameraSource { Usb { device: String }, Network { url: String, secret: Option<SecretRef> } }
pub struct CameraInfo { id: CameraId, name: String, source: CameraSource, enabled: bool, state: CameraState }
pub enum CameraState { Asleep, Opening, Live { width: u32, height: u32, fps: f32 }, Failed(String), Unplugged }
pub struct VideoFrame { camera: CameraId, taken: MediaTime, width: u32, height: u32, rgb: Arc<[u8]> }

#[async_trait]
pub trait CameraHub: Send + Sync {
    async fn list(&self) -> Vec<CameraInfo>;
    async fn add(&self, name: &str, source: CameraSource) -> Result<CameraId, CameraError>;
    async fn remove(&self, id: &CameraId) -> Result<(), CameraError>;
    async fn set_enabled(&self, id: &CameraId, enabled: bool) -> Result<(), CameraError>;
    /// Jedna bieżąca klatka; budzi kamerę, jeśli śpi. `max_side` ogranicza rozmiar.
    async fn frame(&self, id: &CameraId, max_side: u32) -> Result<VideoFrame, CameraError>;
    /// Klatki podglądu (niska rozdzielczość, ograniczona częstość).
    fn preview(&self, id: &CameraId) -> FrameStream;
    /// Wyłącza wszystkie kamery (przełącznik prywatności, kill-switch).
    async fn stop_all(&self);
}
```

Port systemowy w `platform-contract`: `CameraPort` (lista urządzeń, tryby, otwarcie strumienia, zdarzenia
podłączenia). Zdarzenia na magistrali: `camera.added`, `camera.removed`, `camera.state_changed`,
`camera.opened`, `camera.closed` (bez treści obrazu).

## Zależności

`platform-contract` (`CameraPort`; implementacja Windows w `platform-windows-impl`, Media Foundation),
`lib-netguard` (adresy kamer sieciowych), `secrets` (hasła kamer), `core-config`, magistrala zdarzeń.
Zewnętrzne: dekoder strumieni sieciowych jako osobny proces — wybór do ustalenia (wpis w `docs/vendor/` przed
kodem); kandydat: `ffmpeg`, który jest już sidecarem `tools-media`.

## Niezmienniki

- Kamera wyłączona w ustawieniach nie otwiera się nigdy, niezależnie od tego, kto prosi o klatkę.
- Każde otwarcie kamery emituje `camera.opened`, a wskaźnik w interfejsie jest wtedy widoczny.
- Moduł nie zapisuje klatek ani wideo na dysk; klatka żyje w pamięci do zwolnienia przez odbiorcę.
- Hasło kamery nie występuje w konfiguracji, logach, zdarzeniach ani komunikatach błędów.
- Kamera sieciowa: tylko adres z sieci lokalnej i z listy dozwolonych; przekierowania są odrzucane.
- `stop_all` zamyka wszystkie strumienie w czasie mierzonym w milisekundach i wygrywa z każdym oczekującym żądaniem.
- Bezczynny strumień zasypia po czasie z konfiguracji; uśpiona kamera nie zużywa procesora ani sieci.

## Zdolności / uprawnienia

Nowa zdolność Brokera `camera.view` (odczyt klatki). Agentka nie woła huba bezpośrednio — narzędzia `camera.look`
i `camera.who` są w `vision-describe` i `person-recognition` i przechodzą przez Brokera. Dodawanie i usuwanie
kamer — tylko z interfejsu użytkownika.

## Izolacja

Kamery USB: w procesie rdzenia przez port platformy. Kamery sieciowe: dekoder w osobnym procesie bez dostępu do
danych Alfy; awaria dekodera oznacza stan `Failed` jednej kamery, nie programu.

## Budżet zasobów

Do pomiaru (C3.15): procesor i pamięć na strumień uśpiony i aktywny, czas od żądania do klatki (kamera śpiąca
i obudzona), 2 i 4 strumienie naraz. Progi ustala właściciel.

## Konfiguracja (klucze TOML)

`[camera]`: `enabled` (domyślnie `false`), `idle_sleep_s`, `preview_fps`, `max_streams`;
`[[camera.device]]`: `id`, `name`, `kind`, `address` (bez hasła), `enabled`.

## Wkład do UI

Strona Ustawień „Kamery”: lista z podglądem, dodawanie kamery sieciowej (adres, login, hasło do sejfu, test
połączenia), przełącznik każdej kamery i przełącznik główny, nazwy („kuchnia”, „drzwi”), kamera domyślna.
Wskaźnik kamery w belce okna i w pigułce głosowej.

## Testy akceptacyjne

- `ACC-V6-camera-01` — dwie kamery USB i jedna sieciowa działają jednocześnie; odłączenie jednej nie zatrzymuje pozostałych.
- `ACC-V6-camera-02` — wyłączona kamera nie otwiera się na żądanie agentki; odmowa jest w dzienniku.
- `ACC-V6-camera-03` — przełącznik główny i kill-switch zamykają wszystkie strumienie; wskaźnik gaśnie.
- `ACC-V6-camera-04` — po okresie bezczynności strumień zasypia (brak ruchu sieciowego i dekodowania).
- `ACC-V6-camera-05` — hasło kamery nie pojawia się w plikach, logach ani eksporcie `.alfa`.
- `ACC-V6-camera-06` — adres spoza sieci lokalnej albo przekierowanie jest odrzucane.
- `ACC-V6-camera-07` — czas do klatki i zużycie zasobów poniżej progów (progi ustala właściciel).

## Fake

`camera-hub-fake`: wirtualne kamery z nagranych sekwencji klatek i wirtualnym zegarem; sterowane zdarzenia
(odłączenie, błąd, wolna sieć). Fake `CameraPort` w `platform-fake`.

## Otwarte pytania

- Dekoder strumieni sieciowych: `ffmpeg` jako proces czy biblioteka RTSP w Rust — decyzja po próbie (C3.5).
- Wyszukiwanie kamer ONVIF w sieci lokalnej a reguła „bez nasłuchu” — zakres do uzgodnienia z właścicielem.
- Telefon jako kamera — po etapie D (kanał zdalny), osobny szkic.
