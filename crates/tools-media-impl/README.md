# tools-media-impl

tools-media: informacje z nagłówków (lib-media), konwersje przez sidecar ffmpeg w Job Object (lista zamknięta, wymuszony demuxer, tylko protokół file, nowy plik przez dziennik cofania) i odtwarzanie przez voice-audio w kolejce mówienia scheduler-lite z duckingiem — wszystko przez Brokera (fs.read, fs.write).

Implementacja modułu `tools-media`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-media/SPEC.md`](../../docs/modules/tools-media/SPEC.md).

```bash
cargo test -p tools-media-impl
```
