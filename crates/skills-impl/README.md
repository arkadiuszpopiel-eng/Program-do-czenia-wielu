# skills-impl

Implementacja modułu `skills`: `SkillsModule` = `Skills` + `Module` (`module.toml`: `lazy`, `inproc`,
RAM ≤ 2 MB). Rdzeń `SkillLibrary` z kontraktu; ten crate dodaje:

- trwały magazyn wersji (`DirSkillStore`: `skills.json`, zapis atomowy; zmiana liczona na kopii — błąd
  zapisu = brak zmiany i brak zdarzeń), `MemSkillStore` do testów;
- publikację `skills.*` na magistrali (zmiany wymagają uruchomionego modułu);
- `SkillRunner` — uruchamianie przez port `AgentRuntime::start_with` (koperta ≤ roli wywołującej,
  etykieta dla Replay, rodzic = przebieg agentki) i podpowiedzi umiejętności do zadania (z modelem
  albo bez);
- `SkillsDocuments` — `DocumentStore` kategorii `skills` paczki `.alfa` (dokument `skills.json` =
  paczka z SHA-256; zapis importuje jako propozycje, nigdy nie instaluje; usunięcie niczego nie kasuje).

Zależności produkcyjne: tylko `*-contract`. Testy: kontrakt współdzielony, manifest i cykl życia,
trwałość, zdarzenia, runner na `agent-runtime-fake`, dokument `.alfa`.
