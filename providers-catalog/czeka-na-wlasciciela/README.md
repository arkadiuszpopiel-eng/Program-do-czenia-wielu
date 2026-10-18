# Wpisy katalogu czekające na decyzję właściciela

Pliki tutaj NIE trafiają do programu (nie ma ich w `app-modules/src/catalog.rs` ani w sprawdzeniu rejestru
zgodności, które czyta tylko `providers-catalog/*.toml`).

- `azure-openai.toml` — Azure OpenAI (C2.12, wersja 0.0.64). Kod adaptera i testy są w programie. Żeby włączyć:
  wpis `azure-openai` w `docs/compliance/compliance-registry.json` (decyzja właściciela — rejestr zmienia tylko on;
  propozycja: tagi „unknown” jak przy `openai`, commit `b3599c6c` na gałęzi `s5-enterprise-keys`), przeniesienie
  pliku z powrotem do `providers-catalog/` i pozycja w `CATALOG_FILES` w `crates/app-modules/src/catalog.rs`.
