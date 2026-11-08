# voice-dialog-impl

voice-dialog: manifest i re-eksport czysto funkcyjnego automatu rozmowy (barge-in dwustopniowy, backchannel, prefiks, intencje PL, wznawianie, mowa proaktywna).

Implementacja modułu `voice-dialog`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/voice-dialog/SPEC.md`](../../docs/modules/voice-dialog/SPEC.md).

```bash
cargo test -p voice-dialog-impl
```
