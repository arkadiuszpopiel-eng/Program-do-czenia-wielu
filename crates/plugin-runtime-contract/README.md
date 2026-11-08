# plugin-runtime-contract

Wtyczki Wasm (F8-05, ADR 0012): manifest (id, semver, autor, SHA-256 modułu, zdolności ⊆ dozwolone, limity paliwa/czasu/pamięci/I-O), narzędzia z JSON Schema dla rejestru agentek, cykl życia z zatwierdzeniem właściciela z hashem przejrzanej wersji, operacje hosta przez Brokera, zdarzenia plugin.*, zmiana R2 Ulepszacza.

Kontrakt modułu `plugin-runtime`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/plugin-runtime/SPEC.md`](../../docs/modules/plugin-runtime/SPEC.md).

```bash
cargo test -p plugin-runtime-contract
```
