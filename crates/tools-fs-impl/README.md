# tools-fs-impl

tools-fs: narzędzia plikowe agentek przez Brokera (decide → zatwierdzenie → verify) i FsPort, mutacje w dzienniku cofania, deny-listy przed wykonaniem.

Implementacja modułu `tools-fs`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-fs/SPEC.md`](../../docs/modules/tools-fs/SPEC.md).

```bash
cargo test -p tools-fs-impl
```
