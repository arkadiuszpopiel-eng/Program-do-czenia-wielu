# docs/vendor — skróty zależności dla agentów AI

Wiedza modeli budujących projekt kończy się w IV–VI 2026. Biblioteki, których używamy, zmieniają się szybciej
(WASI 0.3, wasmtime 46, Tauri 3 alpha, windows-rs 0.100, tauri-specta RC). Dlatego dla każdej zależności,
której API nie jest oczywiste, trzymamy tu **skrót ≤ 2 strony**: dokładnie te wywołania, których używamy,
zweryfikowane kompilacją.

## Zasady
- Jeden plik na crate/pakiet: `docs/vendor/<nazwa>.md`.
- Nagłówek: przypięta wersja, link do docs.rs / repo, data weryfikacji.
- Treść: tylko używane typy i funkcje z minimalnymi przykładami, znane pułapki, co NIE działa (np. `cpal` a loopback WASAPI).
- Aktualizacja skrótu = część PR-u, który podnosi wersję.
- `windows-rs`: jeden crate, jedna wersja w całym workspace (§4.3 planu).

## Do napisania w F0 (kolejność wg pierwszego użycia)
| Zależność | Powód |
|---|---|
| `tauri` 2.x | okna, IPC, capabilities per okno, tray, efekty okna (Mica) |
| `windows-rs` (przypięta) | UIA (MTA), SendInput, Job Objects, hooki, Credential Manager |
| `wasapi` | loopback, tryb zdarzeniowy, `GetStreamLatency` |
| `whisper.cpp` (FFI/sidecar) | Vulkan/CUDA, VAD, streaming |
| `wasmtime` + `wit-bindgen` | wtyczki Wasm, `wasip2`, limity fuel/epoch |
| `tauri-specta` / `ts-rs` | generowanie typów TS |
| `rusqlite` + SQLCipher + `sqlite-vec` + FTS5 | dane |
| `pulldown-cmark` + `ammonia` | markdown renderowany w Rust |
| Svelte 5 (runes) + Bits UI | UI; zakaz składni Svelte 4 |
