# agent-builder-fake

Kreator agentów (F5): deterministyczna atrapa — ten sam rdzeń co -impl, katalog person i biblioteka w pamięci, nagrane zdarzenia.

Atrapa modułu `agent-builder` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/agent-builder/SPEC.md`](../../docs/modules/agent-builder/SPEC.md).

```bash
cargo test -p agent-builder-fake
```
