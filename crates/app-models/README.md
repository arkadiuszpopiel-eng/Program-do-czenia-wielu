# app-models

Menedżer modeli i silników (kategoria app-*): katalog (LLM, STT, TTS, VAD, KWS, ECAPA, embeddingi, sidecary) z licencją, źródłem i SHA-256, pobieranie HTTPS z Range i wznawianiem, limit równoległości, anulowanie, zgoda TOFU, bezpieczne rozpakowanie ZIP; embedder wyszukiwania (lib-embed / leksykalny) z przebudową wektorów; komendy models_*, embed_model_activate, search_reindex_*.

Warstwa kompozycji aplikacji (`app-*`): składa moduły w całość dla `app-core` i powłoki; nie jest modułem z trójką `contract/impl/fake`.

```bash
cargo test -p app-models
```
