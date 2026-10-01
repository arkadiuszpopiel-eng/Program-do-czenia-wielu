# evals/F7 — pamięć pełna (ACCEPTANCE F7)

| Kryterium | Gdzie | Uruchomienie |
|---|---|---|
| F7-01 izolacja (0 przecieków) | `memory_contract::contract_tests_f7::spy` (3+ sesje z prywatną, 4 agentki, 1000 zapytań) | `cargo test -p memory-impl --test f7_contract`, `-p memory-fake` |
| F7-02 recall@5 ≥ 0,85 | `recall/` (ten katalog), runner `memory_impl::eval` | `cargo test -p memory-impl --test f7_recall -- --nocapture` |
| F7-03 kaskada `forget` | `contract_tests_f7::forget` (50 usunięć zweryfikowanych), property `memory-impl/tests/forget_props.rs` | jak wyżej |
| F7-04 proweniencja (0 awansów w 50) | `contract_tests_f7::access::untrusted_never_promotes`, `memory-consolidation-contract/tests/guardian.rs` | `cargo test -p memory-consolidation-contract` |
| F7-05 konsolidacja nie na baterii / w grze | `memory-consolidation-impl/tests/adapters.rs` (atrapa `device-profile`, 20 scenariuszy) | `cargo test -p memory-consolidation-impl` |
| F7-06 round-trip `.alfa` | `memory-impl/tests/documents.rs` (silnik `transfer`, desktop → laptop); `alfa-full/` — sesja `transfer` | — |
