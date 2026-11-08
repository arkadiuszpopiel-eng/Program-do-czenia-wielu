# vision-describe — SPEC (szkic v0, planowany — plan v6)

## Cel

Opis tego, co widzi kamera: na polecenie „opisz mi to, co widzisz” (albo pytanie o konkretną kamerę) moduł bierze
klatkę z `camera-hub`, wybiera model z obsługą obrazu i zwraca odpowiedź po polsku. Moduł **nie** rozpoznaje
osób (to `person-recognition`) i **nie** prowadzi ciągłej obserwacji. Stan: propozycja, kod nie istnieje.
Opis całości: [`../../plan/12-kamery-i-rozpoznawanie-osob.md`](../../plan/12-kamery-i-rozpoznawanie-osob.md).

## Fala i priorytet

Plan v6, **grupa C3** (zadania C3.6–C3.7). v0: narzędzie `camera.look`, opis jednej klatki modelem lokalnym albo
chmurowym, miniatura w rozmowie. v1: pytania uzupełniające o tę samą klatkę, porównanie z wcześniejszym opisem
(„co się zmieniło”), kilka kamer w jednym pytaniu. Nie wymaga ADR 0025.

## Kontrakt (szkic Rust)

```rust
// szkic — nazwy do ustalenia w SPEC v1
pub struct LookRequest { camera: Option<CameraId>, question: String, privacy: PrivacyTag, max_side: u32 }
pub struct LookAnswer { camera: CameraId, taken: MediaTime, text: String, model: ModelId, thumbnail: ImageRef }
pub enum VisionRoute { Local(ModelId), Cloud { provider: ProviderId, model: ModelId } }

#[async_trait]
pub trait VisionDescribe: Send + Sync {
    /// Czy jest dostępny jakikolwiek model widzący i którą drogą pójdzie zapytanie.
    async fn route(&self, privacy: PrivacyTag) -> Result<VisionRoute, VisionError>;
    async fn look(&self, request: LookRequest) -> Result<LookAnswer, VisionError>;
}
```

Narzędzie agentek: `camera.look { camera?, question }` → tekst opisu (treść oznaczona jako niezaufana).
Zdarzenia: `vision.look_started`, `vision.look_finished` (kamera, model, czas; bez obrazu).

## Zależności

`camera-hub-contract` (klatki), `router-contract` i `providers-contract` (model z wejściem obrazowym),
`safety-broker-contract` (zdolność `camera.view`), `model-residency-contract` (załadowanie modelu widzącego na
czas zapytania), `sessions` (miniatura jako załącznik tury). Zewnętrzne: tryb obrazu `llama-server`
(projektor obrazu modelu) — etykieta pewności i wybór modelu w `docs/plan/11-technologie-2026-10.md`.

## Niezmienniki

- Klatka i jej opis są treścią **niezaufaną**: tekst widoczny w obrazie nie jest poleceniem dla agentki.
- W sesji prywatnej i w profilu „Lokalny” obraz nie opuszcza komputera.
- Obraz trafia do dostawcy w chmurze tylko przy włączonej zgodzie na wysyłanie obrazu z tej kamery.
- Bez modelu widzącego moduł zwraca czytelny błąd z informacją, co pobrać albo włączyć — nie zgaduje opisu.
- Odpowiedź zawsze podaje, z której kamery i z jakiej chwili pochodzi klatka.
- Moduł nie przechowuje klatek po zakończeniu zapytania; miniatura należy do sesji i znika razem z nią.

## Zdolności / uprawnienia

`camera.view` per agentka; poziom ryzyka akcji: niski dla kamery nazwanej przez użytkownika w poleceniu, wyższy,
gdy agentka sama wybiera kamerę (propozycja — do ustalenia z właścicielem razem z politykami Brokera).

## Izolacja

W procesie rdzenia; model lokalny w istniejącym procesie `llama-server` (albo drugim egzemplarzu — do pomiaru).

## Budżet zasobów

Do pomiaru (C3.6, C3.15): czas od polecenia do pierwszego słowa opisu, pamięć karty z modelem rozmowy i modelem
widzącym naraz na 6 GB, koszt zapytania chmurowego z obrazem. Progi ustala właściciel.

## Konfiguracja (klucze TOML)

`[vision]`: `enabled`, `local_model`, `max_side` (domyślny rozmiar klatki), `cloud_allowed` (domyślnie `false`),
`cloud_cameras` (lista kamer, z których obraz może iść do chmury).

## Wkład do UI

Miniatura klatki przy odpowiedzi (z nazwą kamery i godziną), oznaczenie „obraz wysłany do: …” przy drodze
chmurowej, przełącznik zgody na chmurę na stronie „Kamery”, pozycja w osi czasu.

## Testy akceptacyjne

- `ACC-V6-vision-01` — „opisz mi to, co widzisz” zwraca opis zgodny ze sceną testową (zestaw scen: sesja-recenzentka).
- `ACC-V6-vision-02` — w sesji prywatnej żadne żądanie z obrazem nie wychodzi poza komputer.
- `ACC-V6-vision-03` — napis z poleceniem w kadrze nie zmienia zachowania agentki (zestaw ataków w `evals/`).
- `ACC-V6-vision-04` — bez modelu widzącego użytkownik dostaje jasny komunikat, nie zmyślony opis.
- `ACC-V6-vision-05` — czas do pierwszego słowa opisu poniżej progu (próg ustala właściciel).

## Fake

`vision-describe-fake`: stałe opisy przypisane do znanych klatek testowych; sterowane błędy (brak modelu, odmowa
Brokera, przerwa sieci).

## Otwarte pytania

- Który model lokalny z obsługą obrazu mieści się obok modelu rozmowy na 6 GB — pomiar w C3.6.
- Czy opis ma być czytany na głos domyślnie, gdy polecenie padło głosem — propozycja: tak.
