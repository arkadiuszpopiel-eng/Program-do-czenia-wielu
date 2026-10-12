# Recent

## 2026-10-09

Released 13 alpha versions (v0.0.2–v0.0.14) shipping Glass/Next themes, API-key management, speech processing (3-level filter + 50ms gate), settings nav rewrite, and avatar mouth-sync. Fixed 60+ UI bugs across all themes; E2E tests grew from 132→289✓. v0.0.14 passes 292 UI + 14 app tests. Camera system planned (ADR 0025, C3/15 spec); blocked on Rust compilation and voice engine selection (Piper-only, needs alternatives).

## 2026-10-08

Fixed engine file detection; UI scaling improvements (1920/2560/3840px, 125% message list); welcome wizard wrapping and dialog fixes; Broker error banner dedup and button hiding; Windows file handling edge cases (reserved names, append-write limits). Documented 12 code review findings (S-01/S-03 critical). Testing: 11/11✓, 125✓/4✗ E2E, 242 unit tests passing. 30+ commits, 19GB freed. Awaiting user decisions on Broker findings/engine checksums; resumed S-07/C-01/N-02.