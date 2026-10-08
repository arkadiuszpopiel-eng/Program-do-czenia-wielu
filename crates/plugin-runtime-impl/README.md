# plugin-runtime-impl

Wtyczki Wasm (F8-05): piaskownica wasmtime 36 LTS (komponenty WIT, bez WASI), paliwo + epoch + limiter pamięci per wywołanie, nowa instancja na wywołanie, ładowanie tylko po weryfikacji SHA-256, operacje hosta przez Brokera dla agentki wywołującej, narzędzia w rejestrze agentek, magazyn w katalogu.

Implementacja modułu `plugin-runtime`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/plugin-runtime/SPEC.md`](../../docs/modules/plugin-runtime/SPEC.md).

```bash
cargo test -p plugin-runtime-impl
```
