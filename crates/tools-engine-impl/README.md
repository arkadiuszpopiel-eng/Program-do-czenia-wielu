# tools-engine-impl

tools-engine: serwer silnika jako proces poboczny (tylko pętla zwrotna, środowisko bez sekretów, kill-switch), własny klient WebSocket, narzędzia agentek przez Brokera, zrzuty i podglądy poza katalogiem silnika.

Implementacja modułu `tools-engine`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-engine/SPEC.md`](../../docs/modules/tools-engine/SPEC.md).

```bash
cargo test -p tools-engine-impl
```
