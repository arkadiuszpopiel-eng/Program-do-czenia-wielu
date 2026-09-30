# scheduler-lite — SPEC (szkic v0)

## Cel
Deterministyczny, lekki scheduler F2: zasoby wyłączne `speaker` (głośnik/mówienie) i `mic`, kolejka mowy (jedna agentka naraz), timeouty, delegacja v0 = przekazanie tury/rozmowy innej personie bez pracy w tle. Pełny `scheduler` (priorytety, DAG, zasoby ekran/mysz/pliki, cykle, zakleszczenia, okna czasowe) — F5 (PLAN §9.3, §16.2).

## Fala i priorytet
F2. P0. Kontrakt projektowany tak, by pełny `scheduler` (F5) był jego nadzbiorem.

## Kontrakt (szkic Rust)
```rust
// scheduler-lite-contract — SZKIC
pub enum Resource { Speaker, Mic /* F5: ScreenInput, File(PathBuf), ... */ }
pub struct LockRequest { pub resource: Resource, pub holder: Holder /* Persona(PersonaId) | System(ModuleId) */, pub priority: Priority, pub max_wait: Duration, pub preemptible: bool }
pub struct LockGuard { pub id: LockId, pub resource: Resource, pub holder: Holder }
pub trait SchedulerLite: Send + Sync {
    fn acquire(&self, r: LockRequest) -> BoxFuture<Result<LockGuard, SchedError /* Timeout | Preempted | Cancelled */>>;
    fn release(&self, g: LockGuard);
    fn preempt(&self, resource: Resource, by: Holder, reason: PreemptReason /* UserSpeaks | KillSwitch | Handoff */) -> Result<()>;
    fn queue(&self, resource: Resource) -> Vec<QueuedHolder>;
    fn handoff(&self, from: PersonaId, to: PersonaId) -> Result<()>;   // delegacja v0
}
```
Zdarzenia: `sched.acquired/released`, `sched.queued`, `sched.preempted` (powód), `sched.timeout`, `sched.handoff`.

## Zależności
`core-bus/config/log-contract`, `personas-contract`. Klienci: `voice-dialog`, `voice-audio`, `voice-tts`, `notify` (earcony nie biorą `speaker` — miksowane z duckingiem), `agent-runtime` (F3).

## Niezmienniki
- Deterministyczny: brak LLM; ta sama sekwencja żądań = ta sama kolejność przyznań (testowalne).
- `speaker` trzyma najwyżej jeden holder; mowa użytkownika (`UserSpeaks`) wywłaszcza narrację/fillery natychmiast (≤ 5 ms decyzji).
- Kill-switch zwalnia wszystkie blokady i czyści kolejki (wywołanie z `watchdog`, nie z UI).
- `max_wait` zawsze skończony; timeout = błąd z komunikatem, nigdy wieczne czekanie.
- Delegacja v0 nie uruchamia pracy w tle: to przekazanie tury; równoległość dopiero w F5.
- Usługi systemowe (System) nie mają persony i nie mówią; ich raporty relayuje Dyrygentka.

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `always` (krytyczny wg §3.1).

## Budżet zasobów
RAM ≤ 1 MB; decyzja ≤ 1 ms; brak I/O.

## Konfiguracja (klucze TOML)
`[scheduler] speaker.max_wait = "30s"`, `speaker.preempt_on_user_speech = true`, `handoff_phrase = true` („Przekazuję Delcie…").

## Wkład do UI
Kolejka mówienia widoczna w panelu Agentki (kto mówi, kto czeka); kapsuła aktywności.

## Testy akceptacyjne
- `ACC-F2-scheduler-lite-01`: property-based — 1000 losowych sekwencji acquire/release/preempt: 0 podwójnych holderów, 0 zakleszczeń, wszystkie timeouty respektowane.
- `ACC-F2-scheduler-lite-02`: wywłaszczenie przez mowę użytkownika ≤ 5 ms (wirtualny zegar).
- `ACC-F2-scheduler-lite-03`: zmiana obsady/handoff w locie bez restartu sesji (E2E z `personas`).

## Fake
`scheduler-lite-fake`: natychmiastowe przyznania lub skryptowane kolejki/timeouty na wirtualnym zegarze.

## Otwarte pytania
- Czy `scheduler` (F5) zastępuje crate, czy rozszerza kontrakt `-lite` (preferencja: rozszerza; `scheduler-lite-contract` zostaje podzbiorem) — do ustalenia w SPEC v1.
