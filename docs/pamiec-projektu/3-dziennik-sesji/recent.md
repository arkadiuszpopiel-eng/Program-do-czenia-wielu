# Recent

## 2026-10-11

Implemented event sequencing (event_log.rs, event-sequencer.ts) and error-detail refactor; released v0.0.19–0.0.22 (event recovery, API key monitoring w/ failover, IPC type gen, Polish eval cases). Started C2.14 key watch and ChangeHistory UI (e2e 6/6). Alfa 0.0.25–0.0.27 shipped: 15-glos-lokalny plan, Piper fastest (0.56s), Supertonic/Chatterbox slow; Settings/shortcut review fixed CSS vars and Alt+5-7 mismatch; disk cleanup freed ~93GB. Delta 0.8.35 speech port plan (ADR 0027) awaits approval.

## 2026-10-12

Shipped Alfa v0.0.41–0.0.49: duties logic, system selftest, Ctrl+/ search, night report (23-raport-nocny), P1 talk/speech (Supertonic, Qwen ASR, Smart Turn). Added self-test infra (verify.mjs, hardware and auto-eval duties), voice gating/process guard (0.0.48), and onboarding rewrite; clean-profile test found and fixed a terminal login crash. Voice PTT verified and bug fixed (d67e937c); full verification passed (Rust 2680, UI 347, browser 353, real app 27/27); final build pending.