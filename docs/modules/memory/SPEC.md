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

## Zmiany po implementacji (F7 — pamięć pełna, `memory-*` + `memory-consolidation-*`, 2026-10-01)
- **Silnik w kontrakcie.** Cała logika F7 jest w `memory_contract::MemoryEngine<B>` nad portem magazynu
  `MemoryBackend` (wpisy, kandydaci wyszukiwania, transakcja z indeksem, dziennik, notatki eksportów, usunięcie
  zakresu). `memory-impl::SqliteBackend` (SQLCipher + `search`) i `memory-fake::FakeBackend` (mapy) różnią się tylko
  magazynem — atrapa zachowuje się jak implementacja (wzór: silnik `transfer`). Kontrakt v0 (`Memory`) zostaje jako
  **fasada zakresu sesji** (API v0 nie niesie tożsamości wywołującego); `SqliteMemory` v0 działa dalej na tej samej
  bazie (wspólne migracje `0001`, `0002`), pomija wersje zastąpione.
- **Warstwy:** robocza = wpisy przypięte (`pinned`; warstwa `Working` tylko w zakresie sesji, zawsze przypięta) +
  `working_set(sesja, zapytanie, budżet znaków)`; kompaktowanie okna kontekstu zostaje w `agent-runtime`.
  Epizodyczna (zdarzenia, streszczenia), semantyczna (fakty z tematem `subject`), proceduralna (umiejętności).
- **Zakresy i magazyn:** sesja → baza sesji (`SessionDbProvider`); projekt/agentka/globalna → **osobne szyfrowane
  bazy** `memory/{global,project-<id>,agent-<id>}.db` z kluczem `alfa/memory/<zakres>` w `KeyVault`
  (`VaultScopeDbs`; identyfikatory `[A-Za-z0-9_-]{1,64}`). Odczyt nie tworzy pustych baz.
- **Uprawnienia** (`Accessor`): `Owner` (UI) — wszystko; `Guardian` (Strażniczka) — odczyt wszystkiego, zapis tylko
  zmianami konsolidacji i propozycjami awansu (oczekującymi); `Agent(AgentAccess)` — zakresy względne
  `Session/Project/Agent/Global` osobno do odczytu i zapisu (manifest roli; mosty CLI: `read_only`). Zakres
  nieprzyznany → `Forbidden` (nie cicha pustka). Agentka nie nadaje proweniencji `User`; zapis agentki poza sesją
  → wpis oczekujący (zgoda użytkownika). Inspektor, edycja, eksport, import, cofanie — tylko właściciel.
- **Proweniencja:** `Provenance` + `Origin { session, turn, derived_from: [EntryRef], derivation }`
  (`Extracted/Summary/Skill/Promoted/Edited/Imported`), `trusted`, `confidence`, `created_at`.
- **Wersje:** sprzeczność tematu (ten sam `subject`, inna treść) przy zapisie/zatwierdzeniu → nowa wersja
  (`version+1`, `supersedes`), stara zostaje jako `superseded {by, reason, at}`; niższe zaufanie nowszego → konflikt
  w dzienniku. Edycja w Inspektorze = nowa wersja (`Edited`). Stan liczony: `Active/Pending/Superseded/Expired`.
- **Prywatność:** sesja prywatna (`private`, `local_only`; nieznana → prywatna) nigdy nie zasila zakresów szerszych
  (`PrivateSource` przy zapisie z `origin.session`, awansie, zmianach konsolidacji); treść niezaufana — tylko zakres
  sesji, nigdy auto-zapamiętanie, nigdy awans (także przez import i zmiany konsolidacji; pochodna niezaufanego źródła
  jest niezaufana).
- **Recall:** kandydaci = hybryda FTS (rdzenie słów, dopasowanie „dowolne słowo”) + wektor w bazie zakresu
  (`search_contract::TxSearcher`) → filtr aktywnych → **reranking** przez port `Reranker` (domyślnie
  `HeuristicReranker`: pokrycie rdzeni PL z obocznościami, wynik RRF, pewność, przypięcie, zaufanie, świeżość) →
  top-k. Pamięć podręczna 64 wyników / 30 s, czyszczona każdym zapisem.
- **Inspektor:** `inspect` (zakresy, tekst — tylko dopasowanie leksykalne, warstwy, stany, zaufanie, przypięcie, sesja,
  źródło, daty, stronicowanie ≤ 500), `explain` („dlaczego to pamiętam”: powody PL, źródła z istnieniem, historia
  wersji, scalone duplikaty, pochodne, dziennik, wygaśnięcie), `edit`, `set_pinned`, `approve_as`, `export_scope`,
  `scopes`, `journal`, `undo`.
- **`forget` kaskadowo** (`ForgetTarget::{Entry, Scope, Session, Turn, Source}`), plan czysty `plan_cascade`:
  nasiona → (cel „wpis”) rodzina wersji i scalone duplikaty → pochodne, gdy **którekolwiek** źródło znika (wariant
  bezpieczniejszy niż „wyłącznie z tego źródła”; konsolidacja może odtworzyć fakt z pozostałych źródeł) →
  przywrócenie wpisów zastąpionych przez usunięte. Wykonanie: zakresy szersze najpierw; wpis + FTS + wektor + rekordy
  dziennika z migawkami + pamięć podręczna; po usunięciach `secure_delete`, `optimize` FTS5, `wal_checkpoint(TRUNCATE)`;
  zakres własny → crypto-shredding (klucz + pliki); `CascadeReport` z eksportami do ponownego wygenerowania.
  Sesja usuwana w `sessions` → najpierw `forget(Session)` (kopie w zakresach szerszych), potem crypto-shredding bazy.
- **Dziennik i cofanie:** `apply_changes(ChangeSet)` — `Create/Supersede/Resolve/Merge/Expire/MarkConsolidated/
  FlagConflict`, atomowo w zakresie, rekord z migawkami i `run`; `undo` (wygaszenie nieodwracalne — dziennik nie
  trzyma treści wygaszonych).
- **`transfer`:** `MemoryDocuments` = `DocumentStore` kategorii `memory` (`global.ndjson`, `project/<id>.ndjson`,
  `agent/<id>.ndjson`, `session/<id>.ndjson`; linia = wpis z wersjami); pamięć sesji prywatnych nie jest wystawiana;
  zapis = import „dokładnie zawartość” z walidacją całego dokumentu (wiersz łamiący reguły → nic nie zapisano);
  wektory budowane na nowo.
- **Recall@5:** `memory_impl::eval` + `evals/F7/recall/` (249 zapytań PL, format na korpus użytkownika). Wynik na
  `HashEmbedder` (CI, nieblokujący): 0,964 (hybryda), 0,948 (atrapa leksykalna). Próg 0,85 — na prawdziwym embedderze
  w kompozycji `app-*`.
- **Konsolidacja** — osobny moduł `memory-consolidation` (docs/modules/memory-consolidation/SPEC.md).
- Zdarzenia dodatkowe: `memory.approved`, `memory.pinned`, `memory.edited`, `memory.promoted`,
  `memory.changes.applied`, `memory.change.undone`, `memory.exported`, `memory.imported`,
  `memory.consolidation.{started,finished,skipped}` — bez treści.
- Otwarte: produkcyjny embedder wielojęzyczny (ONNX lub `ModelProvider::embed`) i reranker modelowy; stemming PL
  w FTS5 (dziś: rdzenie po stronie pamięci); „dzielenie jawne” między projektami poza awansem.
