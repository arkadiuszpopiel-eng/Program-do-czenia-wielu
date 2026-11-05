---
name: alfa-sprzatanie-target-100gb
description: "Stare kompilacje kasować zawsze (od 5.11): scripts/sprzatanie.sh po verify i backup, limit target/debug 50 GB, nigdy drugiego katalogu target"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 1d67eb46-2bd8-4b76-bbc9-6a5dcd27cf28
  modified: 2026-11-05T14:59:00.276Z
---

Od 5.11.2026 stare kompilacje kasuję **zawsze**, nie dopiero przy awarii. `scripts/sprzatanie.sh` uruchamia się sam
na końcu `scripts/verify.mjs` (każdy poziom; `ALFA_KEEP_BUILD=1` wyłącza na jeden bieg) i `scripts/backup.sh`:
kasuje `apps/desktop/src-tauri/target`, `target/trial-profile`, `target/verify/<starsze wersje>`, a `target/debug`
przez `cargo clean --profile dev`, gdy przekracza `ALFA_TARGET_LIMIT_GB` (domyślnie 50). Zostają `target/owner`
i `target/shell`. Powłokę Tauri buduję **wyłącznie** z `CARGO_TARGET_DIR=target/shell` — nigdy gołe
`cargo … --manifest-path apps/desktop/src-tauri/Cargo.toml` (to tworzy osobne ~50 GB). Na koniec każdej sesji
sprawdzam `du` katalogu `alfa`.

**Why:** 5.11 katalog projektu miał ponad 200 GB (target/debug 87 GB + osobny apps/desktop/src-tauri/target 53 GB
z moich ręcznych uruchomień lintera powłoki i z kroków verify bez wspólnego katalogu). Właściciel: „koniec
z zapychaniem tak dysku! stare kompilacje mają być kasowane”. Wcześniej: 31.10 i 9.10 E: zapełniony (LNK1180).

**How to apply:** po każdej pracy z cargo — `bash scripts/sprzatanie.sh`; przed długim biegiem `df -h /e`.
Wszystko zostaje na E: (decyzja z 15.10, patrz [[alfa-srodowisko-lokalne]]); nie przenosić `target` na C:.
