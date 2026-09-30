# memory — SPEC (szkic v0)

## Cel
Pamięć agentek w czterech warstwach (robocza, epizodyczna, semantyczna, proceduralna) z zakresami `sesja` (domyślny) · `projekt` · `globalna` · `agentka`, operacje `remember / recall / forget` (kaskadowo), proweniencja, pewność, TTL, nocna konsolidacja (PLAN §10, §8.7).

## Fala i priorytet
F1: v0 — `remember/recall` per sesja (semantyczna + epizodyczna w bazie sesji). F7: pełna (4 warstwy, konsolidacja, Inspektor, `forget` kaskadowo, awans zakresów). P0 (v0).

## Kontrakt (szkic Rust)
```rust
// memory-contract — SZKIC
pub enum MemoryScope { Session(SessionId), Project(ProjectId), Global, Agent(PersonaId) }
pub struct MemoryEntry { pub id: MemoryId, pub scope: MemoryScope, pub layer: Layer /* Working | Episodic | Semantic | Procedural */,
    pub text: String, pub entities: Vec<Entity>, pub provenance: Provenance /* User | Agent | UntrustedContent { source } | Import */,
    pub confidence: f32, pub ttl: Option<Duration>, pub created: Timestamp, pub approved: bool }
pub trait Memory: Send + Sync {
    fn remember(&self, e: MemoryEntry, mode: Mode /* Explicit | AutoPendingApproval */) -> Result<MemoryId>;
    fn recall(&self, scope: &[MemoryScope], query: &str, k: usize) -> Result<Vec<MemoryEntry>>;
    fn forget(&self, id: MemoryId) -> Result<ForgetReport>;      // kaskada: embeddingi, streszczenia, kopie, eksporty
    fn promote(&self, id: MemoryId, to: MemoryScope) -> Result<()>; // tylko za zgodą użytkownika
    fn pin(&self, session: SessionId, e: MemoryEntry) -> Result<()>; // warstwa robocza
}
```
Zdarzenia: `memory.remembered`, `memory.pending_approval`, `memory.recalled` (Diagnostics), `memory.forgotten`, `memory.promoted`, `memory.consolidation.started/finished`.

## Zależności
`core-bus/config/log-contract`, `sessions-contract`, `search-contract` (FTS + wektory), `device-profile-contract` (bateria/pełny ekran dla konsolidacji), `model-residency-contract` i `providers-local-contract` (F7, konsolidacja modelami lokalnymi).

## Niezmienniki
- Zakres domyślny = sesja; dzielenie między sesjami tylko jawnie (`promote` za zgodą) — 0 przecieków (testy szpiegowskie).
- Wpisy z `Provenance::UntrustedContent` nigdy nie awansują do `Global`; auto-`remember` z treści niezaufanej wyłączony (PLAN §8.7).
- `forget` jest kaskadowe i weryfikowalne (raport listuje usunięte embeddingi/streszczenia/kopie; eksporty oznaczone do ponownego wygenerowania).
- Szyfrowanie w spoczynku (baza sesji); zakres `Global` w osobnej szyfrowanej bazie.
- Konsolidacja nie startuje na baterii ani w trybie gry/pełnego ekranu; używa tylko modeli lokalnych.
- Mosty CLI mają dostęp do pamięci wyłącznie przez `recall` (PLAN §8.5).

## Zdolności / uprawnienia
Brak własnych tokenów; dostęp agentki do zakresów ograniczony manifestem roli (`memory: { scope, retain }`, PLAN §9.5).

## Izolacja
`inproc`, `always` (warstwa robocza) — moduł krytyczny wg §3.1; konsolidacja jako zadanie tła `on-demand`.

## Budżet zasobów
RAM ≤ 8 MB (bez embeddera); `recall` k=5 ≤ 50 ms na 10k wpisów; konsolidacja tylko w idle.

## Konfiguracja (klucze TOML)
`[memory] default_scope = "session"`, `auto_extract = "ask" | "on" | "off"`, `default_ttl = "90d"`, `[memory.consolidation] enabled = true`, `window = "02:00-05:00"`, `not_on_battery = true`.

## Wkład do UI
Akcja „zapamiętaj" na wiadomości (z wyborem zakresu), `/pamięć`, panel Pamięć / Inspektor pamięci (F7, makieta 9), strona Ustawienia → Pamięć.

## Testy akceptacyjne
- `ACC-F1-memory-01`: kontrakt remember/recall/forget na `-fake`/`-impl`; recall zwraca tylko wpisy z podanych zakresów.
- `ACC-F1-memory-02`: testy szpiegowskie — wpis sesji A niedostępny w sesji B (0/1000).
- `ACC-F7-memory-03`: recall@5 ≥ 0,85 na ≥ 200 zapytaniach PL (zestaw zamrożony); kaskada `forget` zweryfikowana.
- `ACC-F7-memory-04`: wpis z proweniencją niezaufaną nie awansuje do Global (100% odrzuceń).

## Fake
`memory-fake`: wpisy w pamięci z fixture'ów, recall po prostym dopasowaniu, licznik kaskady `forget`, przełącznik „bateria/pełny ekran" dla konsolidacji.

## Otwarte pytania
- Kompaktowanie okna kontekstu (warstwa robocza) — tu czy w `agent-runtime`; do ustalenia w SPEC v1.
- Model ekstrakcji faktów (lokalny 3–4B vs API zależnie od tagu prywatności) — do ustalenia w F7.

## Zmiany po implementacji (v0 F1, `memory-contract/-impl/-fake`, 2026-09-30)
- Kontrakt synchroniczny: `remember(NewMemory, RememberMode) -> MemoryEntry`, `recall(scopes, query, k) ->
  Vec<Recalled { entry, score }>`, `get`, `list` (Inspektor), `approve`, `forget(scope, id) -> ForgetReport`,
  `promote(scope, id, to)`. **`forget`/`promote`/`get` przyjmują zakres**, bo wpis żyje w bazie swojej sesji.
  `pin` (warstwa robocza) — poza v0.
- v0: zakres `Session` i warstwy `Episodic`/`Semantic`; pozostałe → `Unsupported`. `promote`: wpis z
  `UntrustedContent` → `UntrustedCannotPromote` **do każdego zakresu** (surowiej niż „nie do Global”);
  zaufane → `Unsupported` (F7). Automatyczne zapamiętanie z treści niezaufanej → `UntrustedAutoRemember`.
- `MemoryEntry`: `entities: Vec<String>`, `trusted` (kopia proweniencji), `ttl_secs`, `approved` (tryb
  `AutoPendingApproval` → niezatwierdzony, niewidoczny w `recall` do `approve`).
- Magazyn: `memory_entries` w szyfrowanej bazie sesji + dokumenty `DocKind::Memory` w indeksie `search`
  (w tej samej transakcji). `recall` = hybryda RRF przez `Search` wywoływany jako `Caller::Agent{sesja}`,
  potem filtr zatwierdzone/niewygasłe. `forget` = wpis + FTS + wektor w jednej transakcji;
  `ForgetReport { entry, fts_rows, vectors, derived }` (`derived` — streszczenia/kopie od F7, w v0 = 0).
- Atrapa bez przełącznika „bateria/pełny ekran” (konsolidacja poza v0).
- Zdarzenia: `memory.remembered`, `memory.pending_approval`, `memory.recalled` (Debug), `memory.forgotten` —
  bez treści wpisów.
