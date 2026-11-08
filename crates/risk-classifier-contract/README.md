# risk-classifier-contract

Kontrakt klasyfikatora ryzyka (docs/modules/risk-classifier/SPEC.md, PLAN §8.3, §8.7, §6.10).
Deterministyczna, bezstanowa ocena bez LLM: `ActionFacts` (klasa akcji, odwracalność `yes|scoped|no`,
zakres, egress i allowlista, destrukcyjność, masowość, instalacja, źródło polecenia z pewnością STT
w promilach, taint, niezaufane argumenty, dane prywatne, wykryta reguła Jądra) + `AutonomyLevel`
(L0–L4, domyślnie L3) → `RiskVerdict` (klasa `Low/Medium/High/Critical`, werdykt
`Proceed/Ask{non_voice, grantable}/HardBlock{rule}`, reguły, wyjaśnienie po polsku).
Tabela `RULES` (15 reguł) określa zasięg każdej reguły: „każdy poziom, także L4” (twarde blokady Jądra,
destrukcja głosem, niska pewność STT, ryzykowna akcja głosem bez weryfikacji mówcy, admin, trifecta,
egress z sesji tainted) albo „do L1/L2/L3”. Monotoniczność w poziomie autonomii wynika z konstrukcji
i jest sprawdzana testami własności. Reguły „każdego poziomu” nigdy nie są `grantable`
(„zawsze zezwalaj” nie eskaluje do L4). Testy: tabela 80 przypadków × 5 poziomów, 8 własności po 2000
przypadków, współdzielony test kontraktowy (feature `contract-tests`).
