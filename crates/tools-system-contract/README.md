# tools-system-contract

tools-system: kontrakt narzędzi systemowych agentek — procesy (lista, szczegóły, zakończenie z tożsamością i strażnikiem celów), usługi (lista, start/stop przez system.admin), Dziennik zdarzeń (Application/System, filtr, redakcja), zmienne środowiskowe (odczyt z ukryciem sekretów, zapis użytkownika z cofaniem), zasilanie/wyświetlacze/audio; wynik = treść niezaufana.

Kontrakt modułu `tools-system`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/tools-system/SPEC.md`](../../docs/modules/tools-system/SPEC.md).

```bash
cargo test -p tools-system-contract
```
