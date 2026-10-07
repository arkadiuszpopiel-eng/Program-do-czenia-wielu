# voice-persona — SPEC (v1: kontrakt, fake i impl w repo; F2)

## Cel
„Biblia głosu" per agentka (język, postrzegany wiek 18–25, barwa, rejestr, tempo, energia, emocje, słowa-klucze, pochodzenie i zgoda), mapowanie stylu na możliwości silnika, planista emocji, edytowalny słownik wymowy, **normalizator PL** (liczby, daty, skróty, waluty, URL, kod), podział na kanał mówiony (krótki, bez markdown) i ekranowy (PLAN §6.6–6.7).

## Fala i priorytet
F2. P0. Biblie z `docs/PERSONAS.md`.

## Kontrakt (v1, `crates/voice-persona-contract`)
```rust
pub use personas_contract::PersonaId;             // jedno źródło prawdy (re-eksport); walidacja: parse_persona_id(&str) -> Result<PersonaId, PersonaError>
pub struct VoiceBible { persona, display_name, lang, perceived_age /* 18..=25 */, timbre, register, tempo, energy,
    pitch_semitones, default_emotion, emotion_range, voice_prompt, provenance, consent }   // validate(): wiek, „girl/cute/child”, real_person=false
pub struct Lexicon;                              // serde; klucz = słowo/fraza (bez wielkości liter); wymowa bez cyfr i znaczników; Origin: Builtin|User|Improver
pub struct StyleTags { emotion, tempo, energy }  // znaczniki w tekście: [emocja:radość] [tempo:wolno] [energia:wysoka]
pub struct EngineStyleTable { engine, rate, pitch_semitones, gain_db, intensity, emotions }   // tabela per silnik
pub struct SpeechStyle { engine, rate, pitch_semitones, gain_db, intensity, emotion_tag }
pub struct SpokenPlan { persona, sentences: Vec<SpokenSentence { text, tags, style }>, on_screen: Vec<String>, unsupported }
pub struct Chunk { text, boundary: Sentence|Semicolon|Line|Clause|Forced|Code|End }   // ChunkerCfg { max_chars 160, first_max_chars 90, hard_max_chars 300 }
pub trait TextNormalizer { fn normalize(&self, text: &str, lexicon: &Lexicon) -> String; }
pub trait SpeechChunker: Send { fn push(&mut self, delta: &str) -> Vec<Chunk>; fn finish(&mut self) -> Vec<Chunk>; }
pub trait StylePlanner { fn plan_style(&self, bible: &VoiceBible, tags: &StyleTags, table: &EngineStyleTable) -> StylePlan; }
pub trait Persona { fn bible(&PersonaId) -> Result<VoiceBible>; fn normalize_pl(&str) -> String;
    fn plan(&PersonaId, text, &EngineStyleTable) -> Result<SpokenPlan>; fn chunker(ChunkerCfg) -> Box<dyn SpeechChunker>;
    fn lexicon() -> Lexicon; fn set_lexicon_entry(word, pron, Origin) -> Result<()>; fn remove_lexicon_entry(word) -> Result<()>; }
```
Zdarzenia: `voice.persona.plan_created` (Diagnostics), `voice.persona.lexicon_changed` (R0, Audyt gdy `Origin::Improver`), `voice.persona.style_unsupported`.

**Normalizator (impl):** liczebniki 0–999 999 999 999 z rodzajem (dwie minuty, jedno zadanie) i dopełniaczem po przyimkach (od pięciu), większe i z zerami wiodącymi — cyfra po cyfrze; daty (`1 października 2026`, `1.10.2026`, `2026-10-01` → „pierwszego października dwa tysiące dwudziestego szóstego roku”), lata z „r./roku/rok” i po „w”; godziny z przypadkiem wg przyimka (o czternastej, przed dwunastą); waluty zł/PLN/$/USD/€/EUR/£ z groszami i odmianą; tys./mln/mld; procenty; jednostki (km, kg, GB, MB, °C, km/h, ms, min, h…); zakresy („pięć do dziesięciu minut”); wersje; telefony; skróty z odmianą (np., itd., m.in., tzn., dr, ul., godz., nr…); URL („link do github kropka com”), e-mail („… małpa firma kropka pl”), domeny/pliki; kod → „(kod na ekranie)”; PL/EN bez zmian. **Chunker:** koniec zdania/średnik/linia, długie zdania na przecinku, blok kodu w całości; nie tnie po skrótach, inicjałach i w liczbach dziesiętnych; wynik niezależny od podziału strumienia.

## Zależności
`core-bus-contract` (nazwy zdarzeń), `personas-contract` (`PersonaId` — ujednolicony, re-eksport w tym kontrakcie); zamiast `voice-tts-contract::EngineCaps` tabela `EngineStyleTable` w tym kontrakcie. Zewnętrzne: brak.

## Niezmienniki
- Kanał mówiony bez markdown/kodu/tabel — te idą na ekran; model streszcza głosem.
- Normalizator deterministyczny (property-based: idempotentny, nie zmienia słów spoza wzorców), format pl-PL.
- Znaczniki stylu mapowane na możliwości silnika; brak możliwości = pominięcie, nigdy tekst tagu w mowie.
- Słownik wymowy w pierścieniu R0: zmiany `Improver` tylko po bramce ewaluacyjnej i cofalne; zmiany użytkownika natychmiast.
- Biblia zawiera pochodzenie i zgodę; brak klonów prawdziwych osób.
- Język żeński w mowie agentek („zrobiłam"); prompt głosu unika słów „girl/cute/child".

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `lazy`.

## Budżet zasobów
`plan()` ≤ 5 ms na 1000 znaków; RAM ≤ 3 MB (+ słownik).

## Konfiguracja (klucze TOML)
`[voice.persona.<id>] bible = "personas/<id>/voice-bible.toml"`, `tempo = 1.0`, `energy = 0.5`; `[voice.lexicon] file = "personas/lexicon.toml"`, `[voice.normalizer] currency = "PLN"`, `read_urls = "short"`, `read_code = "summary"`.

## Wkład do UI
Ustawienia → Głos → Słownik wymowy (edytor), Ustawienia → Agentki (biblia, próbka głosu), Voice Lab (odsłuch stylów).

## Testy akceptacyjne
- `ACC-F2-voice-persona-01`: normalizator PL — zestaw ≥ 300 przypadków (liczby, daty, skróty, waluty, URL, kod) 100% zgodności; round-trip STT (WER/CER) na wygenerowanej mowie ≤ próg z ACCEPTANCE.
- `ACC-F2-voice-persona-02`: podział mówione/ekranowe — 0 fragmentów markdown/kodu w kanale mówionym na fixture'ach.
- `ACC-F2-voice-persona-03`: property-based idempotencja normalizatora.

## Fake
`voice-persona-fake`: biblie wbudowane, normalizator tabelowy (słownik + cyfry pojedynczo), chunker liniowy, plan bez stylów; przechodzi test kontraktowy.

## Stan testów (v1)
Kontrakt współdzielony (impl + fake), 125 przypadków tabelarycznych normalizatora, property-based (brak cyfr ASCII, idempotencja, identyczność tekstu bez wzorców, chunker niezależny od podziału strumienia), styl per silnik. Pełny zestaw ≥ 300 (ACC-01) tworzy recenzent.

## Otwarte pytania
- Planista emocji: v1 = reguły tabelowe; mały model — po Voice Lab.
- Nazwy stylów chmurowych w tabelach (ElevenLabs/Azure/Cartesia) — zweryfikować z dokumentacją dostawców przy integracji `voice-tts`.
- Format biblii (TOML w `personas/`) i jego eksport w `.alfa` — z `transfer`.
