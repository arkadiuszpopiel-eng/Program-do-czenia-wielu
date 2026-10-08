# agent-runtime-fake

agent-runtime: atrapa — przebiegi ze skryptu zdarzeń (plan, kroki, prośby o zatwierdzenie, pętla, budżet) na wirtualnym zegarze, steering i anulowanie (testy UI, voice-dialog, broker-ui).

Atrapa modułu `agent-runtime` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/agent-runtime/SPEC.md`](../../docs/modules/agent-runtime/SPEC.md).

```bash
cargo test -p agent-runtime-fake
```
