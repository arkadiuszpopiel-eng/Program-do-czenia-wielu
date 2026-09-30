# memory-contract

Kontrakt pamięci v0 (docs/modules/memory/SPEC.md): `Memory` (`remember`, `recall`, `get`, `list`, `approve`,
`forget`, `promote`), zakresy `Session | Project | Global | Agent` (v0: tylko sesja), warstwy (v0:
epizodyczna i semantyczna), `Provenance` (`User`, `Agent`, `UntrustedContent`, `Import`), pewność, TTL,
`ForgetReport`. Reguły wspólne: `validate_new` (m.in. brak auto-zapamiętywania z treści niezaufanej),
`recall_sessions`, `is_expired`, `check_promotion` (niezaufane nie awansują nigdzie). `contract_tests`
(feature): przywołanie, izolacja sesji (0/1000), kaskada `forget`, proweniencja, zatwierdzanie i TTL,
walidacja.
