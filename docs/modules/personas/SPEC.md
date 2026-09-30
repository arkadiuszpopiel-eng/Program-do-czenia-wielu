# personas — SPEC (szkic v0)

## Cel
Cztery persony na start (Alfa, Beta, Gama, Delta: imię + głos + charakter + fraza + kolor) i **obsada ról** (Dyrygentka, Mówczyni, Myślicielka, Wykonawczyni, Koderka, Krytyczka, Badaczka, Strażniczka pamięci, Pisarka/Tłumaczka + własne) per sesja/zadanie/szablon; szablony obsad Standard/Solo/Kodowanie/Badania; zmiana obsady z UI, paska sesji, głosem lub przez Marszałka (PLAN §9.2, `docs/PERSONAS.md`).

## Fala i priorytet
F2 (persony + obsada; rola = prompt + polityka modelu). F3: tokeny zdolności per rola (Broker). F5: Kreator agentek (`agent-builder`). P0.

## Kontrakt (szkic Rust)
```rust
// personas-contract — SZKIC
pub struct Persona { pub id: PersonaId /* alfa|beta|gama|delta|custom */, pub name: String, pub glyph: char /* α β γ δ */, pub color: ColorToken,
                     pub character: String, pub wake_phrase: String, pub voice: VoiceRef, pub system_prompt: PromptRef /* język żeński */ }
pub struct Role { pub id: RoleId, pub name: String, pub prompt: PromptRef, pub model_policy: ModelPolicy, pub tools: Vec<ToolId>,
                  pub permission_profile: PermissionProfileRef /* F3 */, pub memory_scope: MemoryScopePolicy, pub read_only: bool /* Krytyczka */ }
pub struct Cast { pub id: CastId, pub name: String, pub assignments: Vec<(PersonaId, Vec<RoleId>)>, pub conductor: PersonaId, pub speaker: PersonaId }
pub trait Personas: Send + Sync {
    fn personas(&self) -> Vec<Persona>;
    fn roles(&self) -> Vec<Role>;
    fn cast(&self, session: SessionId) -> Cast;
    fn set_cast(&self, session: SessionId, cast: Cast, origin: Origin) -> Result<()>;   // natychmiast, do dziennika
    fn resolve_addressee(&self, session: SessionId, name: Option<&str>) -> PersonaId;   // imię wygrywa, inaczej Dyrygentka
}
```
Zdarzenia: `personas.cast.changed` (Audyt: kto, skąd — UI/głos/Marszałek), `personas.role.assigned`, `personas.persona.added` (Kreator, F5).

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
