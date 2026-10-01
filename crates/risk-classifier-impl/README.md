# risk-classifier-impl

Klasyfikator ryzyka (`TableClassifier`): tabela z `risk-classifier-contract` z progami polityki Jądra
(`[security.risk] stt_confidence_min`, `bulk_threshold`; zmiana wyłącznie z `KernelAuthority` Brokera,
publikuje `risk.rules.changed`), zdarzenia diagnostyczne `risk.classified` (przy `Ask`/`HardBlock`)
i `risk.trifecta_detected`, implementacja `Module` (rejestr, `module.toml`). Działa w procesie Brokera;
kopia read-only w jądrze służy do podglądu „dlaczego pyta”. Zależy wyłącznie od crate'ów `*-contract`.
