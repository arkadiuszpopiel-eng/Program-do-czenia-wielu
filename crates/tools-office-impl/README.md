# tools-office-impl

tools-office: odczyt i edycja Word/Excel przez Brokera (fs.read, fs.write, gui.control) i OfficePort — kopia robocza zapisywana jako nowa wersja przez dziennik cofania, makra wyłączone (fail-closed), treść niezaufana.

Implementacja modułu `tools-office`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-office/SPEC.md`](../../docs/modules/tools-office/SPEC.md).

```bash
cargo test -p tools-office-impl
```
