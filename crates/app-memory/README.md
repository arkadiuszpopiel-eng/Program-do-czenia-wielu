# app-memory

Pamięć F7 w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit rozmiaru crate'a).

- `memory_module` — `memory-impl::MemoryModule`: zakres sesji w szyfrowanej bazie sesji, pozostałe
  zakresy w bazach `%LOCALAPPDATA%\Alfa\memory` (klucze w sejfie `StoreKeyVault`), prywatność sesji
  z katalogu sesji (`CatalogPrivacy`).
- `RoleAccess` — `Accessor::Agent` z ról obsady sesji i projektu sesji (Strażniczka — wszystko;
  Dyrygentka/Głos — odczyt wszystkiego, zapis sesja+agentka; Krytyczka — tylko odczyt sesji i projektu).
- `tools` — narzędzia agentek `memory_recall` / `memory_remember` (treść niezaufana → taint wyniku).
- `guardian` — Strażniczka pamięci (`memory-consolidation-impl`): klucze `memory.consolidation_enabled`,
  `memory.consolidation_window` (domyślnie `02:00-05:00`), `memory.auto_extract`; licznik bezczynności
  z monitora sygnałów platformy (`app-core`), bez monitora `UnknownIdle` („nigdy bezczynny"; ręczne
  „Porządkuj teraz" działa).
- `MemoryApp` — komendy Inspektora (`memory_*`), `remember_turn`, `forget_session` (przed
  crypto-shreddingiem), kontekst czatu (`system_prompt`), most zdarzeń `memory.*` → `MemoryChanged`.
- `MemoryDocuments` — dokumenty `Category::Memory` dla paczek `.alfa` (sesje prywatne poza eksportem).

Identyfikator wpisu w DTO: `"<klucz zakresu>#<id>"` (`session:<id>`, `project:<id>`, `agent:<id>`, `global`).
Testy integracyjne: `crates/app-core/tests/memory.rs`, `spy_work.rs`.
