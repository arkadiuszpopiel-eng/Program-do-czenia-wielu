# wasmtime (piaskownica wtyczek Wasm) — `plugin-runtime-impl`

- **`wasmtime` =36.0.16** (Apache-2.0 WITH LLVM-exception) — **linia LTS 36.x** (wsparcie do ~VIII 2027),
  MSRV 1.86 ≤ toolchain 1.94. Nowsze linie odpadają: 47.x ma **niezałatane RUSTSEC-2026-0315/0316**
  (paliwo przy `call_ref`/wyjątkach, alokacja przy podnoszeniu rekordów), 48.x/49.x wymagają Rust 1.95/1.96.
  Przy podbijaniu: tylko patch w 36.x albo nowe LTS po zmianie toolchainu (ADR), zawsze `cargo deny check`.
  36.0.17 (2026-10-02, wydanie zbiorcze z 48.0.4/49.0.2 — RUSTSEC-2026-0325…0327 nie dotyczą < 39) ma
  identyczne źródła `wasmtime`, `wasmtime-environ`, `wasmtime-internal-cranelift/-unwinder`,
  `cranelift-codegen/-frontend`, `pulley-interpreter` (diff 0 linii) — podbić przy następnej łatce 36.x.
- Cechy: `default-features = false, features = ["runtime", "cranelift", "component-model", "std"]` —
  bez `cache`, `wat`, `profiling` (ittapi: GPL-2.0 OR BSD-3), `parallel-compilation`, `pooling-allocator`,
  `gc*`, `threads`, `async`, `coredump`, `debug-builtins`, `addr2line`, `demangle`, `stack-switching`.
  `Component::new` bez `wat` przyjmuje wyłącznie binaria.
- Testy: `wat = "=1.236.1"` (dev; ta sama linia wasm-tools co `wasmparser 0.236` w wasmtime 36) — wtyczki
  z WAT kompilowane w teście, bez binariów w repo. `docs.rs` niedostępny w sandboksie → API sprawdzone w
  źródłach rejestru i kompilacją (2026-10-03, `cargo test -p plugin-runtime-impl`).
- Pomiar (pusty `target/`, 4 vCPU, profil dev workspace): wasmtime z powyższymi cechami **+535 MB**,
  **47 s** (≈150 s CPU; najdłużej `cranelift-codegen` 22 s); dla porównania `wasmi 2.0.0` (interpreter,
  MIT/Apache-2.0): +63 MB, 10 s — odrzucony: brak modelu komponentów (ADR 0012: WIT), wymagałby nowego ADR.
  Binarka testowa `plugins` ≈ 64 MB (wszystkie testy impl w jednym pliku binarnym).

## Używane API (36.x)
```rust
let mut c = wasmtime::Config::new();
c.consume_fuel(true).epoch_interruption(true).max_wasm_stack(512 * 1024)
 .wasm_component_model(true).wasm_relaxed_simd(false).wasm_memory64(false).wasm_multi_memory(false)
 .wasm_custom_page_sizes(false).wasm_wide_arithmetic(false).wasm_stack_switching(false)
 .cranelift_opt_level(wasmtime::OptLevel::Speed);         // wasm_threads/gc: brak metod bez cech
let engine = wasmtime::Engine::new(&c)?;                    // engine.increment_epoch() z wątku-zegara
let mut linker = wasmtime::component::Linker::<Data>::new(&engine);
linker.instance("alfa:plugin/host@0.1.0")?
    .func_wrap("call", |mut cx: StoreContextMut<'_, Data>, (op, args): (String, String)|
        -> wasmtime::Result<(Result<String, String>,)> { … })?;   // Err(..) = pułapka u gościa
let comp = Component::from_binary(&engine, bytes)?;          // moduł rdzeniowy → błąd parsera komponentów
for (name, item) in comp.component_type().imports(&engine) { /* ComponentItem::ComponentInstance(i) → i.exports(&engine) */ }
// ComponentItem::ComponentFunc(f): f.params() → (&str, Type), f.results() → Type; Type::Result(r): r.ok()/r.err()
let pre = linker.instantiate_pre(&comp)?;                    // kontrola typów importów; InstancePre: Clone
let mut store = Store::new(pre.engine(), data);
store.limiter(|d| &mut d.limiter);                           // ResourceLimiter: memory_growing/table_growing (usize)
store.set_fuel(n)?; store.get_fuel()?;
store.epoch_deadline_callback(|mut cx| /* Ok(UpdateDeadline::Continue(1)) | Err(..) */);
store.set_epoch_deadline(1);
let inst = pre.instantiate(&mut store)?;
let f = inst.get_typed_func::<(&str, &str), (Result<String, String>,)>(&mut store, "invoke")?;
let (out,) = f.call(&mut store, (tool, input))?; f.post_return(&mut store)?;
err.downcast_ref::<wasmtime::Trap>()   // OutOfFuel | StackOverflow | Interrupt | MemoryOutOfBounds | …
```

## Pułapki
- `wasmtime::Error` = `anyhow::Error`: własny powód przerwania (`thiserror`) przez `Error::new(..)` i
  `downcast_ref` — działa także, gdy wasmtime doklei kontekst z backtrace'em. Komunikat pułapki może
  zawierać nazwy funkcji z modułu (dane niezaufane) → zawsze `sanitize` + `root_cause()`.
- Błąd z limitera przy `memory_growing` = pułapka (zapamiętaj flagę, żeby rozpoznać „limit pamięci”);
  `Ok(false)` = `memory.grow` zwraca -1. Pamięć początkowa ponad limit = błąd `instantiate`.
- Callback epoki wołany co krok zegara tylko podczas wykonania Wasm; czas operacji hosta odejmujemy sami
  (`started.elapsed() - host_time`). `Engine::increment_epoch` jest globalne dla silnika — termin per `Store`.
- `Component::serialize/deserialize` (pamięć podręczna na dysku) — `deserialize` jest `unsafe` → zakazane
  (`unsafe_code = forbid`); kompilujemy przy ładowaniu i trzymamy `InstancePre` w pamięci (klucz SHA-256).
- Typed `func_wrap`/`get_typed_func` dla `result<string,string>`: krotka `(Result<String, String>,)`.
- Komponent z WAT: pamięć i `realloc` w osobnym module rdzeniowym (bez cyklu `canon lower` ↔ instancja).
- Wtyczki w Rust: `wasm32-unknown-unknown` + `wit-bindgen` + `wasm-tools component new` (bez adaptera WASI).
  `wasm32-wasip2` ze `std` importuje `wasi:cli/*`, `wasi:io/*` → odrzucone przy ładowaniu (brak importów WASI).
- `Linker` klonowany + użycie po zwolnieniu (RUSTSEC-2026-0090) dotyczy ≥ 43 — w 36.x nie występuje;
  i tak nie klonujemy linkera.
