---
name: alfa-sprzatanie-target-100gb
description: "Gdy pliki budowania (alfa/target) przekroczą 100 GB — sprzątać od razu, zanim dysk E: się zapełni"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 1d67eb46-2bd8-4b76-bbc9-6a5dcd27cf28
  modified: 2026-10-31T18:12:57.040Z
---

Gdy `alfa/target` (głównie `target/debug`) przekroczy **100 GB**, sprzątam od razu, bez pytania:
`cargo clean --profile dev` (z `source scripts/portable-env.sh`). Zostają `target/owner` (kopia właściciela),
`target/shell` (build testowy), `target/safety`, `target/release`, `target/verify`. Tych nie ruszam.
Sprawdzam rozmiar (`du -sh target/debug`, `df -h /e`) przed każdym pełnym `cargo test --workspace` i po nim,
a także przed `verify`.

**Why:** 31.10.2026 pełny przebieg testów zapełnił E: (zostało 288 MB, target/debug 168 GB). Linker padł z LNK1180
i testy trzeba było powtarzać. Właściciel: „zapisz sobie, że masz sprzątać, jeśli pliki przekroczą 100 GB, by to się
nie powtarzało”. To samo zdarzyło się już 2026-10-09.

**How to apply:** pilnować progu 100 GB w trakcie pracy, a nie dopiero po błędzie linkera. Wszystko zostaje na E:
(decyzja z 15.10, patrz [[alfa-srodowisko-lokalne]]); nie przenosić `target` na C:.
