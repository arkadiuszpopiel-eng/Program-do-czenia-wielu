# voice-persona-contract

Kontrakt modułu `voice-persona` (PLAN §6.6–6.7, VOICE.md §12). Zawiera:
- `PersonaId` (Alfa/Beta/Gama/Delta + persony z Kreatora), `VoiceBible` z walidacją niezmienników
  (wiek 18–25, brak słów „girl/cute/child” w prompcie, brak prawdziwej osoby) i biblie wbudowane z `docs/PERSONAS.md`;
- `Lexicon` — edytowalny słownik wymowy (serde, klucze bez rozróżniania wielkości liter, wymowa bez cyfr i znaczników, `Origin` User/Improver/Builtin);
- typy chunkera (`Chunk`, `Boundary`, `ChunkerCfg`), planisty stylu (`StyleTags` → `SpeechStyle` przez `EngineStyleTable`) i planu (`SpokenPlan`: kanał mówiony + ekranowy);
- traity `Persona`, `TextNormalizer`, `SpeechChunker`, `StylePlanner`;
- nazwy zdarzeń `voice.persona.*` oraz — pod feature `contract-tests` — `contract_tests::run_all` uruchamiany na `-impl` i `-fake`.

Inne moduły zależą wyłącznie od tego crate'a.
