# platform-windows-ocr-impl

platform-windows (F6): OcrPort dla tools-vision przez Windows.Media.Ocr (WinRT) — dekodowanie BitmapDecoder z kontrolą wymiarów przed pikselami, skalowanie do limitu silnika i współrzędne w pikselach wejścia; poza Windows „nieobsługiwane”.

Implementacja modułu `platform-windows-ocr`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

```bash
cargo test -p platform-windows-ocr-impl
```
