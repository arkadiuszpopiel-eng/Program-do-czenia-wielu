# tools-media-contract

tools-media: kontrakt narzędzi multimediów agentek — informacje z nagłówków (lib-media, bez ffmpeg), konwersje przez opcjonalny sidecar ffmpeg (Transcoder, lista zamknięta formatów, nowy plik przez dziennik cofania) i odtwarzanie dźwięku przez voice-audio w kolejce mówienia (AudioPlayer).

Kontrakt modułu `tools-media`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/tools-media/SPEC.md`](../../docs/modules/tools-media/SPEC.md).

```bash
cargo test -p tools-media-contract
```
