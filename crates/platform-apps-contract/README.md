# platform-apps-contract

Porty aplikacji i systemu dla computer use (F6): Office przez COM (OfficePort, makra zawsze wyłączone, formuły z listy dozwolonej), izolowana przeglądarka z CDP przez potok (BrowserPort, filtr egressu, deny-lista profili), rejestr tylko do odczytu (RegistryPort, deny-lista kluczy z sekretami).

Kontrakt modułu `platform-apps`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/platform-apps/SPEC.md`](../../docs/modules/platform-apps/SPEC.md).

```bash
cargo test -p platform-apps-contract
```
