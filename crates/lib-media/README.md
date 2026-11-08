# lib-media

Wspólna biblioteka (bez logiki modułu): czyste parsery nagłówków multimediów w Rust bez ffmpeg (PNG, JPEG, GIF, BMP, WebP, WAV, MP3, AAC/ADTS, FLAC, Ogg, MP4/MOV/M4A/HEIF, Matroska/WebM, AVI) z budżetem odczytu — złośliwy plik nie wymusi czytania ani pętli bez końca.

Biblioteka pomocnicza (`lib-*`) — bez stanu modułu i bez `module.toml`.

```bash
cargo test -p lib-media
```
