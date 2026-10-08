# tools-screen-impl

tools-screen: zrzut okna/monitora/obszaru na żądanie przez Brokera (gui.control) i ScreenCapturePort z maskowaniem, PNG base64, taint.

Implementacja modułu `tools-screen`. Zależy od kontraktów innych modułów, nigdy od ich `-impl`.

Specyfikacja: [`docs/modules/tools-screen/SPEC.md`](../../docs/modules/tools-screen/SPEC.md).

```bash
cargo test -p tools-screen-impl
```
