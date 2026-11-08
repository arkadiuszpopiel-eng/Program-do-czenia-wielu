# tools-media-fake

tools-media: atrapy — narzędzia z prawdziwymi manifestami i walidacją (wyniki skryptowane), deterministyczny Transcoder (nagłówki formatu docelowego z lib-media, zapis zadań, brak ffmpeg na żądanie) i AudioPlayer zapisujący klipy.

Atrapa modułu `tools-media` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-media/SPEC.md`](../../docs/modules/tools-media/SPEC.md).

```bash
cargo test -p tools-media-fake
```
