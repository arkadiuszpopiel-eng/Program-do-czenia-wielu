# accounts-hub-fake

Atrapy do testów huba kont i modułów, które od niego zależą (`router`, `providers-*`, `cost-meter`):

- `MemorySecretStore` — `SecretStore` w pamięci (wartości zerowane przy zwolnieniu), `fail_next` do ścieżek błędów;
- `ScriptedConnectionTester` / `ScriptedModelLister` — kolejka wyników (`push`) albo reguła prefiksu
  klucza (`sk-ok`, `sk-bad`, `sk-rate`, `sk-net` z kluczem w komunikacie — sprawdza redakcję, `sk-slow` — 60 s);
- `MapEnv` — `EnvSource` z mapy;
- `FakeAccountsHub` — `AccountsHub` w pamięci: identyfikatory `acc-1`, `acc-2`…, zegar sterowany
  (`set_now`), wymuszanie stanu (`set_state`), rejestr odczytów kluczy (`secret_reads`).

Przechodzi ten sam zestaw `contract_tests::run_all` co `accounts-hub-impl`. Bez magistrali, plików
i sieci. Tylko jako `dev-dependency` innych modułów.
