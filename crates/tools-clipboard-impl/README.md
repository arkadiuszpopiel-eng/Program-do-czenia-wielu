# tools-clipboard-impl

tools-clipboard: odczyt/zapis schowka (tekst, obraz PNG) przez Brokera i ClipboardPort, odczyt jako niezaufana treść (taint), cofanie zapisu z wykrywaniem konfliktu.

Implementacja modułu `tools-clipboard`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-clipboard/SPEC.md`](../../docs/modules/tools-clipboard/SPEC.md).

```bash
cargo test -p tools-clipboard-impl
```
