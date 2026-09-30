# personas — SPEC (v1: kontrakt zaimplementowany)

## Cel
Cztery persony na start (Alfa, Beta, Gama, Delta: imię + głos + charakter + fraza + kolor) i **obsada ról** (Dyrygentka, Mówczyni, Myślicielka, Wykonawczyni, Koderka, Krytyczka, Badaczka, Strażniczka pamięci, Pisarka/Tłumaczka + własne) per sesja/zadanie/szablon; szablony obsad Standard/Solo/Kodowanie/Badania; zmiana obsady z UI, paska sesji, głosem lub przez Marszałka (PLAN §9.2, `docs/PERSONAS.md`).

## Fala i priorytet
F2 (persony + obsada; rola = prompt + polityka modelu). F3: tokeny zdolności per rola (Broker). F5: Kreator agentek (`agent-builder`). P0.

## Kontrakt (źródło prawdy: `crates/personas-contract`)
```rust
pub struct Persona { id: PersonaId, name, glyph: char, color: ColorToken /* "color.agent.alfa" — nazwa tokenu ui-kit */,
                     character, wake_phrases: Vec<String>, forms: NameForms /* 7 przypadków: Delta/Delty/Delcie/Deltę/Deltą/Delcie/Delto */,
                     voice: VoiceBible /* wiek 18–25, barwa, rejestr, tempo, energia, emocje, prompt, pochodzenie */, builtin }
pub struct Role { id: RoleId, name, description, prompt /* żeński */, model_policy, tools, read_only, untrusted_isolated, author, unique, builtin }
pub struct Cast { template: Option<TemplateId>, voice: bool, assignments: BTreeMap<PersonaId, BTreeSet<RoleId>> } // Dyrygentka/Mówczyni wynikają z przydziału
#[async_trait] pub trait Personas: Send + Sync {
    fn personas(&self) -> Vec<Persona>; fn roles(&self) -> Vec<Role>; fn templates(&self) -> Vec<CastTemplate>;
    fn cast(&self, s: &SessionId) -> Cast;                                                   // domyślny szablon, gdy brak
    async fn set_cast(&self, s, cast, origin: ChangeOrigin) -> Result<CastChange, PersonasError>;
    async fn set_voice(&self, s, voice: bool, origin) -> Result<CastChange, PersonasError>;   // bez Mówczyni → Dyrygentka
    async fn apply_command(&self, s, text: &str, origin) -> Result<Option<CastChange>, PersonasError>;
    fn resolve_addressee(&self, s, text: &str) -> PersonaId;                                 // imię wygrywa, inaczej Dyrygentka
    fn system_prompt(&self, s, p: &PersonaId) -> Result<String, PersonasError>;
    async fn add_persona(&self, p: Persona) -> Result<(), PersonasError>;                     // + add_role, add_template (Kreator: model + walidacja)
    fn export(&self) -> PersonasExport;                                                      // `.alfa`: elementy własne + obsady
}
```
Czyste funkcje (wspólne dla `-impl`/`-fake`, dostępne dla `voice-dialog`/`voice-wake`): `Catalog::validate_cast`, `parse_addressee`,
`parse_cast_command` + `apply_command`, `render_system_prompt`, `Cast::verifier_for`; rdzeń stanu `PersonasState`.
Zdarzenia: `personas.cast.changed` (origin: ui/voice/text/marshal/system/import; before/after/assigned/removed/warnings), `personas.role.assigned` (per para), `personas.persona.added`.
Decyzje v1: `SessionId` z `core-bus-contract`; „Krytyczka ≠ autorka, gdy możliwe” = ostrzeżenie walidacji + zastępczyni w `verifier_for`
(nie błąd — „Delta, przejmij weryfikację” w Standard jest poprawne); szablon *Kodowanie* ma Alfę jako Mówczynię; definicje z TOML — F2 (config).

## Zależności
`core-bus/config/log-contract`, `sessions-contract`, `voice-persona-contract` (biblie), `router-contract` (polityka modelu roli), `safety-broker-contract` (F3: nowe tokeny przy zmianie obsady), `ui-kit` (kolory, glify).

## Niezmienniki
- Persona stała (imię, głos, charakter); rola zmienna. **Głos idzie za personą, uprawnienia za rolą** pod sufitem sesji (§8.3).
- Zmiana obsady natychmiastowa, bez restartu sesji, zapisana w dzienniku; przy zmianie Broker wydaje nowe tokeny (F3).
- Krytyczka tylko odczyt; Badaczka na niezaufanych źródłach w izolacji; Wykonawczyni ma narzędzia systemowe.
- Zwrot po imieniu zawsze wygrywa; bez imienia odpowiada Dyrygentka; mówi jedna naraz.
- Prompty w języku żeńskim; usługi systemowe (Scheduler, Marszałek, Diagnosta, Ulepszacz, Watchdog, Router) bez persony i głosu.
- Persona nie może zmienić własnej roli ani obsady bez polecenia użytkownika (Marszałek działa na Twoje polecenie).

## Zdolności / uprawnienia
Brak własnych; profile uprawnień ról to polityka Jądra (tylko Broker/Ty).

## Izolacja
`inproc`, `always` (mały).

## Budżet zasobów
RAM ≤ 1 MB; `resolve_addressee` ≤ 1 ms.

## Konfiguracja (klucze TOML)
`[personas.<id>] name, glyph, color, character, wake_phrase, voice`, `[roles.<id>] prompt, model_policy, tools, read_only`, `[casts.<id>] assignments, conductor, speaker`, `[personas] default_cast = "standard"`.

## Wkład do UI
Awatary obsady w pasku (świeci mówiąca/pracująca), panel Agentki / obsada ról (makieta 6), `@agentka`, `/obsada`, chip roli przy wiadomości („Delta · Wykonawczyni"), Ustawienia → Agentki.

## Testy akceptacyjne
- `ACC-F2-personas-01`: zmiana obsady w locie bez restartu sesji (E2E, w tym głosem „Beta, teraz ty prowadzisz").
- `ACC-F2-personas-02`: property-based `resolve_addressee` — imię zawsze wygrywa; brak imienia → Dyrygentka obsady.
- `ACC-F3-personas-03`: po zmianie obsady tokeny Krytyczki są tylko-odczyt (0/100 zapisów).

## Fake
`personas-fake`: persony/role/obsady z fixture'ów, zapis wywołań `set_cast`.

## Otwarte pytania
- Format promptów persony/roli (pliki w `personas/`, pierścień R0) — do ustalenia w SPEC v1 z `improver`.
- Szablony obsad „Badania" i własne — treść w `docs/PERSONAS.md`.
