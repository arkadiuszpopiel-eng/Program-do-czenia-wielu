# agent-builder-contract

Kontrakt Kreatora agentów (docs/modules/agent-builder/SPEC.md, PLAN §9.5).

- `AgentDraft` (formularz/rozmowa) → `build_manifest` → `AgentManifest` (persona z odmianą imienia
  `decline_feminine`, glif, kolor z palety, biblia głosu; rola z promptem w rodzaju żeńskim; głos v0
  `VoicePreset` odrębny od istniejących; limity: autonomia, budżet, zakresy zapisu, pamięć, wyzwalacze).
- `BuilderPolicy`: lista dozwolonych grup narzędzi (grupy ról wbudowanych), znaczniki Jądra
  (`KERNEL_MARKERS`) w grupach/identyfikatorach, frazy obejścia zabezpieczeń w promptach
  (`PROMPT_PHRASES`, `PROMPT_TOKENS`), sufit autonomii = poziom sesji, nigdy L4, sufit budżetu,
  zakresy zapisu tylko w profilu (`check_fs_scope`).
- `from_description` (deterministycznie) / `DraftLlm` + `from_conversation` (model uzupełnia braki,
  autonomia niższa z dwóch); `preview`, `dry_run` (skryptowany przebieg bez modelu i skutków).
- `BuilderCore` (wspólny dla `-impl`/`-fake`): zapis dwufazowy — `prepare_save` (manifest = dokładnie to,
  co zbudowałby Kreator; hash przejrzany; zaliczony test na sucho; nie głosem) → `commit`.
- `samples`, `contract_tests` (feature): ścieżka szczęśliwa i 41 prób ataku (0 sukcesów).
