# marshal-contract

Kontrakt Marszałka F5 (docs/modules/marshal/SPEC.md, PLAN §9.4): język reguł (`Rule`: `when` →
`then: Vec<Effect>`; tylko efekty zawężające, ścisłe parsowanie), `check_rule` względem `Ceiling`,
`conflicts`, `compose` → `EffectivePolicy` (nigdy szersza niż sufit — `within`), `RuleBook`
(propozycje, zatwierdzanie i cofanie **tylko przez użytkownika**), nadzór `Watch` (eskalacje ze zdarzeń
`scheduler.*`/`triggers.*`, raport dzienny), `MarshalCore<H>` + trait `Marshal`, port `RuleTranslator`
(LLM → niezaufane szkice). Feature `contract-tests`: wspólny zestaw + `ScriptedTranslator`.
