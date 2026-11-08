# tools-shell-impl

tools-shell: polecenia powłoki przez Brokera (shell.exec z regułami Jądra, net.egress dla poleceń sieciowych), snapshot zakresu w dzienniku cofania, Job Object przez ExecPort, środowisko bez sekretów, rejestr kill-switcha.

Implementacja modułu `tools-shell`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-shell/SPEC.md`](../../docs/modules/tools-shell/SPEC.md).

```bash
cargo test -p tools-shell-impl
```
