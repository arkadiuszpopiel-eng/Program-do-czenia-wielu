# platform-apps-fake

Atrapa portów aplikacji (F6): Office z dokumentem w pamięci (makra liczone, Protected View, formuły z listy dozwolonej), przeglądarka z wirtualną siecią i filtrem egressu, rejestr z deny-listą sekretów, system (procesy z drzewem i strażnikiem celów, usługi, zdarzenia, zmienne) i kwarantanna pobrań w pamięci.

Atrapa modułu `platform-apps` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/platform-apps/SPEC.md`](../../docs/modules/platform-apps/SPEC.md).

```bash
cargo test -p platform-apps-fake
```
