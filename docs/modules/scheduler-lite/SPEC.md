# scheduler-lite — SPEC (v1: kontrakt zaimplementowany)

## Cel
Deterministyczny, lekki scheduler F2: zasoby wyłączne `speaker` (głośnik/mówienie) i `mic`, kolejka mowy (jedna agentka naraz), timeouty, delegacja v0 = przekazanie tury/rozmowy innej personie bez pracy w tle. Pełny `scheduler` (priorytety, DAG, zasoby ekran/mysz/pliki, cykle, zakleszczenia, okna czasowe) — F5 (PLAN §9.3, §16.2).

## Fala i priorytet
F2. P0. Kontrakt projektowany tak, by pełny `scheduler` (F5) był jego nadzbiorem.

## Kontrakt (źródło prawdy: `crates/scheduler-lite-contract`)
```rust
pub enum Resource { Speaker, Mic, ScreenInput, File(String /* znormalizowana ścieżka */) }
pub enum Holder { User, Persona(PersonaId), System(String) }
pub enum Priority { Background, Narration, Normal, Interactive, UserSpeech, Critical }
pub struct ResourcePolicy { preemptible_at_atomic: bool, on_timeout: OnTimeout /* AskUser | Fail */, handoff_reserve_ms: u64 }
pub struct LeaseRequest { resource, holder, priority, max_wait: Duration /* ≤ 600 s; 0 = próba */, on_timeout: Option<OnTimeout> }
pub struct Lease { .. } // RAII: drop = release; signal(): Active | PreemptRequested{by, reason} | Revoked{reason}; handoff(to)
#[async_trait] pub trait SchedulerLite: Send + Sync {
    async fn acquire(&self, r: LeaseRequest) -> Result<Lease, SchedError /* Timeout{on_timeout} | Deadlock | Cancelled | AlreadyHeld | SystemCannotSpeak | … */>;
    fn preempt(&self, r: &Resource, by: Holder, reason: PreemptReason /* UserSpeaks | HigherPriority | Handoff | KillSwitch */) -> Result<(), SchedError>;
    fn handoff(&self, r: &Resource, from: &Holder, to: Holder) -> Result<(), SchedError>;   // delegacja v0, bez luki
    fn holder(&self, r: &Resource) -> Option<LeaseInfo>; fn queue(&self, r: &Resource) -> Vec<QueuedRequest>;
    fn kill_all(&self) -> usize;
}
```
Rdzeń decyzyjny w kontrakcie (`LockTable` + sterownik `Core<H: Host>`): `-impl` (zegar tokio, timer, magistrala) i `-fake` (wirtualny zegar) różnią się tylko `Host`.
Zdarzenia: `scheduler.lease.granted/released/preempted/timeout` + `queued/handoff/deadlock/revoked/cancelled`.
Zakres v1 poszerzony względem v0 (zadanie F2): także `ScreenInput` i `File`, wykrywanie zakleszczeń (cykl w grafie oczekiwania → błąd dla najmłodszego żądania),
wywłaszczanie tylko w punktach atomowych (sygnał, nie zabijanie). DAG, okna czasowe i limity współbieżności — pełny `scheduler` (F5) jako nadzbiór.

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
- `scheduler` (F5) rozszerza kontrakt `-lite` (typy `Resource/Holder/Priority/Lease` zostają); DAG i okna czasowe dokłada F5.
- Klucze `[scheduler]` z `core-config` → `ResourcePolicy` (na razie `set_policy`) — po ustabilizowaniu `core-config-contract`.

## Rozszerzenia dla pełnego `scheduler` (F5, addytywne)
`LockTable::is_free_for` / `grant_all` i `Core::try_acquire_all` / `all_free_for` / `policy` — atomowe przyznanie kompletu zasobów
(wszystko albo nic, bez czekania; zajęte → `Timeout { waited_ms: 0 }`). Zadania pełnego schedulera nie czekają w kolejce `-lite`
(brak „hold and wait”); żądania mowy mają pierwszeństwo (zasób z niepustą kolejką nie jest wolny dla zadań).
