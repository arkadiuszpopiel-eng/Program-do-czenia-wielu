# Recent

## 2026-10-12

Shipped Alfa v0.0.41–0.0.49: duties logic, system selftest, Ctrl+/ search, night report, P1 talk/speech (Supertonic, Qwen ASR, Smart Turn). Added self-test infra, voice gating (0.0.48), onboarding rewrite, terminal login fix. Full verification passed (Rust 2680, UI 347, browser 353, real app 27/27). 0.0.50 verified 09:06 but not merged after E: disk disconnects (local commit 99367ee7; 09:18 note: unverified). Shared model queue across accounts in progress; Alfa model ladder assigned, not started. Screenshot tests at 6 theme combos.

## 2026-10-14

Alfa 0.0.51–0.0.58: audit A-01…A-17 fixed (0.0.51–0.0.52); Settings "Dobór modelu" and "Stan połączeń", `docs/DOWODY.md` evidence register, size gate in lint; Antigravity (`agy`) backend and Low-integrity process module (wasmtime 36.0.18). 0.0.53 passed full check (two runs, exit 0) and is installed; 0.0.54 fixed after a red run. 0.0.55–0.0.56 add Google Gemini login via Antigravity CLI (separate login folder, auto-detect, plan in Stan połączeń); 0.0.56 installed (32 real-app tests green), but its full check was interrupted by E: USB disconnects. Agent stage 1 (`system_software`) green; stage 2 (MCP servers) committed 11:57; stage 3 (browser control) started; `pc-diag-contract` crate created, tests not run. Open: 10:00 joint Antigravity test not run (needs owner's Google account); vehicle-photo preview regression (10:37) unfixed; move of `target` to C: awaiting confirmation; AppContainer isolation (A-07), 20 ms budget, Antigravity compat approval.

## Identity Candidates
(none)