# tools-system-impl

tools-system: narzędzia systemowe agentek przez SysPort i Brokera — procesy (zakończenie gui.control(obraz) ze strażnikiem celów i tożsamością), usługi (system.admin), Dziennik zdarzeń, zmienne środowiskowe (ukryte sekrety, zapis użytkownika z cofaniem), zasilanie/monitory/audio; wynik = treść niezaufana.

Implementacja modułu `tools-system`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-system/SPEC.md`](../../docs/modules/tools-system/SPEC.md).

```bash
cargo test -p tools-system-impl
```
