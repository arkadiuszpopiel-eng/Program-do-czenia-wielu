# tools-vision-fake

tools-vision: atrapy — narzędzia z prawdziwymi manifestami i walidacją (wyniki skryptowane), deterministyczny OcrPort (skrypt/stały tekst, zapis obrazów), DescribePort z regułą prywatności (sesja prywatna bez modelu lokalnego → odmowa) i mapa prywatności sesji.

Atrapa modułu `tools-vision` — deterministyczna, tylko w `dev-dependencies` innych modułów; przechodzi te same testy kontraktowe co implementacja.

Specyfikacja: [`docs/modules/tools-vision/SPEC.md`](../../docs/modules/tools-vision/SPEC.md).

```bash
cargo test -p tools-vision-fake
```
