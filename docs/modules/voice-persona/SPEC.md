# voice-persona — SPEC (szkic v0)

## Cel
„Biblia głosu" per agentka (język, postrzegany wiek 18–25, barwa, rejestr, tempo, energia, emocje, słowa-klucze, pochodzenie i zgoda), mapowanie stylu na możliwości silnika, planista emocji, edytowalny słownik wymowy, **normalizator PL** (liczby, daty, skróty, waluty, URL, kod), podział na kanał mówiony (krótki, bez markdown) i ekranowy (PLAN §6.6–6.7).

## Fala i priorytet
F2. P0. Biblie z `docs/PERSONAS.md`.

## Kontrakt (szkic Rust)
```rust
// voice-persona-contract — SZKIC
pub struct VoiceBible { pub persona: PersonaId, pub lang: Lang, pub perceived_age: (u8, u8) /* 18..=25 */, pub timbre: String, pub register: Register,
    pub tempo: f32, pub energy: f32, pub emotion_range: Vec<Emotion>, pub prompt_keywords: Vec<String>, pub provenance: Provenance, pub consent: ConsentRecord }
pub struct SpokenPlan { pub spoken: Vec<Sentence { text_normalized: String, style: StyleTags }>, pub on_screen: Option<String> /* kod, tabele */ }
pub trait Persona: Send + Sync {
    fn bible(&self, p: PersonaId) -> VoiceBible;
    fn plan(&self, p: PersonaId, assistant_text: &str, engine_caps: &EngineCaps) -> SpokenPlan;   // normalizacja + podział + styl→silnik
    fn normalize_pl(&self, text: &str) -> String;
    fn lexicon(&self) -> Lexicon;                                          // słownik wymowy (edytowalny, R0)
    fn set_lexicon_entry(&self, word: &str, pron: &str, origin: Origin) -> Result<()>;
}
```
Zdarzenia: `persona.plan.created` (Diagnostics), `persona.lexicon.changed` (R0, Audyt gdy `Origin::Improver`), `persona.style.unsupported` (silnik bez tagu → degradacja).

## Zależności
`core-bus/config/log-contract`, `personas-contract` (tożsamość, kolory, frazy), `voice-tts-contract` (EngineCaps). Zewnętrzne: brak (normalizator własny, testowany na korpusie).

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
`voice-persona-fake`: biblie z fixture'ów, normalizator przepuszczający (lub tabelowy), plan bez stylów — do testów `voice-tts`/`voice-dialog`.

## Otwarte pytania
- Planista emocji (reguły vs mały model) — do ustalenia w SPEC v1.
- Format biblii (TOML w `personas/`) i jego eksport w `.alfa` — z `transfer`.
