# skills-fake

Umiejętności (F5): deterministyczna atrapa — ten sam rdzeń biblioteki co -impl, magazyn w pamięci, wirtualny zegar, nagrane zdarzenia.

Atrapa modułu `skills` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/skills/SPEC.md`](../../docs/modules/skills/SPEC.md).

```bash
cargo test -p skills-fake
```
