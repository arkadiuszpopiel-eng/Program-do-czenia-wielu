# tools-vision-impl

tools-vision: OCR zrzutu/obszaru/pliku i opis obrazu modelem z wizją przez Brokera (gui.control, fs.read) — zrzut zamaskowany w porcie przed OCR i przed modelem, plik z deny-listą i wymiarami z nagłówka przed dekodowaniem, sesja prywatna tylko lokalnie, treść niezaufana.

Implementacja modułu `tools-vision`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-vision/SPEC.md`](../../docs/modules/tools-vision/SPEC.md).

```bash
cargo test -p tools-vision-impl
```
