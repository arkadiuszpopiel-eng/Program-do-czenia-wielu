# search — SPEC (szkic v0)

## Cel
Wyszukiwanie pełnotekstowe (FTS5) i wektorowe (sqlite-vec) w tej samej szyfrowanej bazie per sesja; „szukaj w rozmowie" i „szukaj wszędzie" jako funkcja UI dla właściciela; embeddingi lokalne (PLAN §1.2, §10, §14.8).

## Fala i priorytet
F1 (FTS w sesjach, wyszukiwanie sesji). Wektory i embeddingi wielojęzyczne dla pamięci — F1 v0 (`recall`), pełne w F7. P0.

## Kontrakt (szkic Rust)
```rust
// search-contract — SZKIC
pub struct Doc { pub id: DocId, pub session: SessionId, pub kind: DocKind /* Turn | MemoryEntry | Artifact */,
                 pub text: String, pub ts: Timestamp, pub meta: Meta }
pub struct Query { pub text: String, pub sessions: SessionSet /* One | All(owner_only) */,
                   pub mode: Mode /* Fts | Vector | Hybrid */, pub limit: usize, pub filters: Filters }
pub struct Hit { pub doc: DocId, pub session: SessionId, pub score: f32, pub snippet: String }
pub trait Search: Send + Sync {
    fn index(&self, doc: Doc) -> Result<()>;
    fn remove(&self, id: DocId) -> Result<()>;                 // kaskada z forget/delete
    fn query(&self, q: Query, caller: Caller) -> Result<Vec<Hit>>;
}
pub trait Embedder: Send + Sync { fn embed(&self, texts: &[String]) -> Result<Vec<Vector>>; fn dims(&self) -> usize; }
```
Zdarzenia: `search.indexed`, `search.removed`, `search.query` (tylko Diagnostics, bez treści), `search.embedder.loaded/unloaded`.

## Zależności
`core-bus/config/log-contract`, `sessions-contract` (dostęp do bazy sesji), `model-residency-contract` (F2, ładowanie embeddera). Silnik: SQLite + FTS5 + sqlite-vec (ADR 8; zgodność z SQLCipher — spike i).

## Niezmienniki
- Indeks żyje w bazie sesji: brak osobnego, nieszyfrowanego indeksu; usunięcie klucza sesji unieważnia indeks.
- `SessionSet::All` dozwolone wyłącznie dla `Caller::Owner` (UI); agentka (`Caller::Agent`) widzi tylko własną sesję/zakres pamięci (PLAN §10: wyszukiwanie między sesjami nigdy nie jest narzędziem agentki).
- Embedder lokalny; brak wysyłania tekstu do chmury bez tagu prywatności zezwalającego i jawnej konfiguracji.
- Wyniki są deterministyczne dla tego samego indeksu i zapytania (stabilne sortowanie).

## Zdolności / uprawnienia
Brak własnych (działa na bazach otwartych przez `sessions`).

## Izolacja
`inproc`, `lazy`; embedder przez `model-residency` (ONNX na CPU), zwalniany po bezczynności.

## Budżet zasobów
FTS zapytanie ≤ 30 ms na 100k tur; wyniki palety ≤ 16 ms/znak (z `ui-shell`); embedder ≤ 300 MB RAM, ładowany na żądanie.

## Konfiguracja (klucze TOML)
`[search] mode = "hybrid"`, `snippet_chars = 160`, `[search.embedder] model = "<do ustalenia w F1>"`, `idle_unload = "5m"`.

## Wkład do UI
`Ctrl+F` (w rozmowie), `Ctrl+Shift+F` (wszędzie), wyszukiwarka panelu Sesje, paleta `Ctrl+K`, Inspektor pamięci (F7).

## Testy akceptacyjne
- `ACC-F1-search-01`: test kontraktowy — index/remove/query, tryby Fts/Vector/Hybrid na `-fake` i `-impl`.
- `ACC-F1-search-02`: izolacja — `Caller::Agent` z sesji A nie dostaje trafień z sesji B (0/1000 prób).
- `ACC-F1-search-03`: kaskada — po `remove`/usunięciu sesji 0 trafień.
- `ACC-F7-search-04`: recall@5 ≥ 0,85 na ≥ 200 zapytaniach PL (zestaw zamrożony).

## Fake
`search-fake`: indeks w pamięci (proste dopasowanie + kosinus na wektorach z fixture'ów), deterministyczne wyniki; fake embedder zwraca wektory z hasha tekstu.

## Otwarte pytania
- Wybór modelu embeddingów wielojęzycznych (jakość PL) — pomiar w F1/F7, do ustalenia w SPEC v1.
- Tokenizacja FTS5 dla polskiego (unicode61 + stemming?) — do ustalenia w SPEC v1.
