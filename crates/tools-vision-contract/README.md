# tools-vision-contract

tools-vision: kontrakt narzędzi wizji agentek — OCR zrzutu/obszaru/pliku (port OcrPort) i opis obrazu modelem z wizją przez Router z tagiem prywatności sesji (DescribePort); maskowanie jak w tools-screen przed OCR i przed modelem, treść niezaufana.

Kontrakt modułu `tools-vision`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/tools-vision/SPEC.md`](../../docs/modules/tools-vision/SPEC.md).

```bash
cargo test -p tools-vision-contract
```
