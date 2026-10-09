# person-recognition — SPEC (szkic v0, planowany — plan v6)

## Cel

Rozpoznawanie zapisanych osób po twarzy: wykrycie twarzy w klatce, policzenie wzorca, porównanie z wzorcami
osób, które same się zapisały, i wynik „kto to, z jaką pewnością”. Moduł **nie** przechowuje pamięci o osobach
(to `person-memory`), **nie** ocenia emocji, wieku ani płci i **nie** tworzy wzorców osób niezapisanych.
Rozpoznanie służy wyłącznie personalizacji rozmowy — **nigdy** autoryzacji. Stan: propozycja, kod nie istnieje;
wymaga podpisu [ADR 0025](../../ADR/0025-kamery-i-biometria-twarzy.md).
Opis całości: [`../../plan/12-kamery-i-rozpoznawanie-osob.md`](../../plan/12-kamery-i-rozpoznawanie-osob.md).

## Fala i priorytet

Plan v6, **grupa C3** (zadania C3.8–C3.10, C3.13). v0: zapis osoby w kreatorze, rozpoznanie jednej osoby przed
kamerą, narzędzie `camera.who`. v1: kilka osób w kadrze, łączenie z głosem (`voice-speaker`), odporność na
zdjęcie pokazane do kamery.

## Kontrakt (szkic Rust)

```rust
// szkic — nazwy do ustalenia w SPEC v1
pub struct PersonId(pub Uuid);
pub struct FaceTemplate(Vec<f32>);              // wzorzec — nigdy poza modułem w postaci jawnej
pub struct Sighting { person: Option<PersonId>, confidence: f32, camera: CameraId, taken: MediaTime }
pub enum EnrollStep { NeedFrontal, NeedLeft, NeedRight, Done { quality: f32 }, Rejected(String) }

#[async_trait]
pub trait PersonRecognition: Send + Sync {
    /// Zapis osoby: kolejne klatki z kreatora; wzorzec powstaje dopiero po `Done`.
    async fn enroll(&self, person: PersonId, frame: &VideoFrame) -> Result<EnrollStep, RecognitionError>;
    async fn forget(&self, person: &PersonId) -> Result<(), RecognitionError>;
    /// Kto jest w kadrze (zapisani) i ile jest twarzy nieznanych.
    async fn identify(&self, frame: &VideoFrame) -> Result<Vec<Sighting>, RecognitionError>;
    /// Bieżący rozmówca z połączenia twarzy i głosu; `None` = nikt pewny.
    fn speaker(&self) -> Option<Sighting>;
}
```

Narzędzie agentek: `camera.who { camera? }` → imiona rozpoznanych osób i liczba nieznanych (bez wzorców).
Zdarzenia: `person.seen`, `person.left`, `person.speaker_changed`, `person.enrolled`, `person.forgotten`.

## Zależności

`camera-hub-contract` (klatki), `person-memory-contract` (rejestr osób), `voice-speaker-contract` (drugi
sygnał — głos), `secrets` (klucz szyfrujący wzorce), `safety-broker-contract`. Zewnętrzne: ONNX Runtime na
procesorze (już w stosie) z modelem wykrywania twarzy i modelem wzorca — **wybór modeli po sprawdzeniu licencji**
(część popularnych modeli twarzy ma licencję tylko do badań); etykiety pewności w
`docs/plan/11-technologie-2026-10.md`.

## Niezmienniki

- Wzorzec powstaje wyłącznie w kreatorze zapisu, po wyraźnej zgodzie osoby; nigdy w tle.
- Wzorce są zaszyfrowane w spoczynku; klucz w Windows Credential Manager. Nie trafiają do logów, zdarzeń,
  diagnostyki ani eksportu `.alfa`; zdjęcia z kreatora nie są zapisywane.
- Wynik poniżej progu pewności to „osoba nieznana” — moduł nie zgaduje.
- Rozpoznanie nie jest dowodem dla Brokera: nie zatwierdza akcji, nie podnosi autonomii, nie odblokowuje sekretów.
- Rozbieżność twarzy i głosu obniża pewność do „nieznana”; agentka pyta zamiast zakładać.
- `forget` usuwa wzorce nieodwracalnie i natychmiast przestaje rozpoznawać osobę.
- Moduł nie liczy żadnych cech poza tożsamością zapisanych osób (żadnych emocji, wieku, płci).

## Zdolności / uprawnienia

`camera.view` + `person.identify` per agentka. Zapis i usuwanie osób — tylko z interfejsu, przez człowieka.

## Izolacja

W procesie rdzenia, obliczenia na procesorze. Rozpoznawanie w tle działa tylko, gdy użytkownik je włączył
i kamera jest włączona; pobiera jedną klatkę w małej rozdzielczości co kilka sekund.

## Budżet zasobów

Do pomiaru (C3.15): czas wykrycia i wzorca na klatkę, zużycie procesora przy rozpoznawaniu w tle, trafność
i odsetek fałszywych rozpoznań na zestawie testowym. Progi ustala właściciel.

## Konfiguracja (klucze TOML)

`[person]`: `recognition` (domyślnie `false`), `interval_s`, `threshold`, `greet` (powitanie po imieniu),
`combine_voice`.

## Wkład do UI

Kreator „Zapisz osobę” (zgoda, imię, trzy ujęcia, wynik jakości), lista osób z usuwaniem, wskaźnik rozmówcy
w belce („rozmawiasz jako: Arek”), przełącznik rozpoznawania w tle.

## Testy akceptacyjne

- `ACC-V6-person-01` — zapisana osoba jest rozpoznawana po restarcie programu (trwałość wzorców).
- `ACC-V6-person-02` — dwie zapisane osoby nie są mylone; osoba niezapisana jest „nieznana” (progi: właściciel).
- `ACC-V6-person-03` — zdjęcie twarzy pokazane do kamery nie daje żadnego uprawnienia (rozpoznanie ≠ zgoda).
- `ACC-V6-person-04` — wzorców nie ma w eksporcie `.alfa`, logach ani paczce diagnostycznej.
- `ACC-V6-person-05` — usunięcie osoby kasuje wzorce; ponowne pojawienie się = „nieznana”.
- `ACC-V6-person-06` — bez zgody w kreatorze wzorzec nie powstaje (test ścieżki przerwanej).

## Fake

`person-recognition-fake`: tożsamość przypisana do znanych klatek testowych, sterowana pewność, scenariusze
„dwie osoby”, „zdjęcie”, „rozbieżny głos”.

## Otwarte pytania

- Modele wykrywania i wzorca z licencją dopuszczającą użycie w produkcie — do sprawdzenia przed C3.8.
- Wykrywanie żywej twarzy (ochrona przed zdjęciem) — czy w v1, czy wystarczy zasada „rozpoznanie ≠ zgoda”.
- Wzorce na osobę: ile ujęć i jak odświeżać, gdy wygląd się zmienia.
