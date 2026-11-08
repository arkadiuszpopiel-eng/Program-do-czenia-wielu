# person-memory — SPEC (szkic v0, planowany — plan v6)

## Cel

Rejestr osób i pamięć długoterminowa per osoba: kim jest dana osoba, jak się do niej zwracać, co o niej wiadomo
i **o czym Alfa z nią rozmawiała**. Po ponownym uruchomieniu programu wszystko jest na miejscu. Moduł **nie**
rozpoznaje twarzy ani głosu (dostaje gotowy identyfikator osoby z `person-recognition`) i **nie** zastępuje
pamięci Alfy — dodaje do niej wymiar „czyje to”. Stan: propozycja, kod nie istnieje; wymaga podpisu
[ADR 0025](../../ADR/0025-kamery-i-biometria-twarzy.md).
Opis całości: [`../../plan/12-kamery-i-rozpoznawanie-osob.md`](../../plan/12-kamery-i-rozpoznawanie-osob.md).

## Fala i priorytet

Plan v6, **grupa C3** (zadania C3.9, C3.11–C3.12). v0: rejestr osób, profil, przypisanie tur do osoby,
streszczenie rozmowy per osoba, pamięć osoby w kontekście. v1: fakty per osoba z konsolidacji nocnej, pamięć
wspólna dla wskazanych osób, przegląd i poprawianie pamięci osoby w interfejsie.

## Kontrakt (szkic Rust)

```rust
// szkic — nazwy do ustalenia w SPEC v1
pub struct Person { id: PersonId, name: String, address_as: String, lang: String, notes: String, created: DateTime }
pub struct PersonFact { person: PersonId, text: String, source: TurnRef, at: DateTime }
pub struct TalkSummary { person: PersonId, session: SessionId, at: DateTime, text: String }
pub struct PersonContext { person: Person, facts: Vec<PersonFact>, recent: Vec<TalkSummary> }

#[async_trait]
pub trait PersonMemory: Send + Sync {
    async fn people(&self) -> Vec<Person>;
    async fn add(&self, name: &str) -> Result<PersonId, PersonError>;
    async fn update(&self, person: &Person) -> Result<(), PersonError>;
    /// Usuwa osobę razem z jej pamięcią (fakty, streszczenia); wzorce usuwa `person-recognition`.
    async fn remove(&self, id: &PersonId) -> Result<(), PersonError>;
    /// Znacznik „kto mówił” dla nowej tury (część tury — historia pozostaje tylko do dopisywania).
    async fn attribute(&self, turn: TurnRef, person: Option<PersonId>) -> Result<(), PersonError>;
    /// Pamięć do kontekstu rozmowy z daną osobą, w budżecie tokenów.
    async fn context(&self, id: &PersonId, budget_tokens: u32) -> Result<PersonContext, PersonError>;
    async fn summarize(&self, id: &PersonId, session: SessionId) -> Result<TalkSummary, PersonError>;
}
```

Zdarzenia: `person.added`, `person.updated`, `person.removed`, `person.memory_changed`.

## Zależności

`memory-contract` (istniejąca pamięć Alfy: warstwy, zakresy, tagi prywatności), `sessions-contract` (tury),
`memory-consolidation-contract` (wyciąganie faktów), `person-recognition-contract` (identyfikator rozmówcy),
`router-contract` (streszczenia — klasa lekkich zadań). Dane w bazie programu (SQLite), jak reszta pamięci.

## Niezmienniki

- Pamięć osoby X trafia do kontekstu tylko wtedy, gdy rozmówcą jest X (albo właściciel jawnie o nią poprosi).
- Gość (osoba nieznana) nie widzi i nie zmienia pamięci żadnej zapisanej osoby.
- Historia rozmowy jest tylko do dopisywania: znacznik osoby powstaje razem z turą; korekta = nowy wpis.
- Sesja prywatna nie zasila pamięci osoby (tak jak nie zasila pamięci Alfy).
- Usunięcie osoby usuwa jej fakty i streszczenia; tury w sesjach zostają, ale tracą przypisanie do osoby.
- Każdy fakt ma źródło (turę), z której pochodzi — da się go sprawdzić i usunąć.
- Eksport `.alfa` obejmuje profil i pamięć osoby tylko po zaznaczeniu tego zakresu; wzorców nie obejmuje nigdy.

## Zdolności / uprawnienia

Odczyt pamięci osoby przez agentkę — istniejąca zdolność pamięci zawężona do bieżącego rozmówcy. Dodawanie,
poprawianie i usuwanie osób — z interfejsu, przez człowieka.

## Izolacja

W procesie rdzenia, w tej samej bazie co pamięć Alfy (osobne tabele, ten sam mechanizm kopii i migracji).

## Budżet zasobów

Do pomiaru: czas złożenia kontekstu osoby, rozmiar pamięci po 100 rozmowach, czas streszczenia po rozmowie.
Progi ustala właściciel.

## Konfiguracja (klucze TOML)

`[person.memory]`: `enabled`, `context_tokens` (budżet pamięci osoby w kontekście), `summaries_kept`,
`guest_talk` (czy osoba nieznana może rozmawiać).

## Wkład do UI

Strona Ustawień „Osoby”: lista, profil, pamięć osoby (fakty ze źródłami, streszczenia rozmów) z poprawianiem
i usuwaniem; w rozmowie znacznik „kto mówił” przy turze; filtr sesji po osobie.

## Testy akceptacyjne

- `ACC-V6-person-memory-01` — po restarcie programu Alfa zna zapisane osoby i streszczenia rozmów z nimi.
- `ACC-V6-person-memory-02` — rozmowa z osobą B nie ujawnia faktów zapisanych o osobie A (zestaw pytań: sesja-recenzentka).
- `ACC-V6-person-memory-03` — „o czym ostatnio rozmawialiśmy?” daje streszczenie rozmów **tej** osoby.
- `ACC-V6-person-memory-04` — usunięcie osoby usuwa jej pamięć; kopia zapasowa sprzed usunięcia ją przywraca.
- `ACC-V6-person-memory-05` — sesja prywatna nie zostawia śladu w pamięci osoby.
- `ACC-V6-person-memory-06` — gość nie dostaje pamięci żadnej osoby.

## Fake

`person-memory-fake`: pamięć w RAM z dwiema osobami testowymi i gotowymi streszczeniami; sterowany zegar.

## Otwarte pytania

- Pamięć wspólna (np. domownicy) — kto decyduje, co jest wspólne.
- Kilka osób w jednej rozmowie: jedna sesja z turami różnych osób czy osobne sesje.
- Jak długo trzymać streszczenia i kiedy je scalać (konsolidacja nocna).
