//! Współdzielony test kontraktowy `Persona` (feature `contract-tests`), uruchamiany na
//! `voice-persona-impl` i `voice-persona-fake`.

use crate::{
    Boundary, CODE_ON_SCREEN, ChunkerCfg, EngineKind, EngineStyleTable, MAX_AGE, MIN_AGE, Origin,
    Persona, PersonaError, PersonaId, SpeechChunker,
};

fn has_ascii_digit(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit())
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cztery persony wbudowane mają poprawne biblie; nieznana → błąd.
pub fn builtin_bibles<P: Persona>(p: &P) {
    for id in PersonaId::builtin() {
        let bible = p.bible(&id).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(bible.persona, id);
        assert!((MIN_AGE..=MAX_AGE).contains(&bible.perceived_age));
        bible.validate().unwrap_or_else(|e| panic!("{e}"));
    }
    let unknown = PersonaId::new("omega").unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(
        p.bible(&unknown),
        Err(PersonaError::UnknownPersona { .. })
    ));
}

/// Wpis słownika ma pierwszeństwo przed regułami; walidacja i usuwanie działają.
pub fn lexicon_priority_and_validation<P: Persona>(p: &P) {
    p.set_lexicon_entry("Tauri", "tałri", Origin::User)
        .unwrap_or_else(|e| panic!("{e}"));
    let out = p.normalize_pl("Używam Tauri i tauri.");
    assert!(out.contains("tałri") && !out.contains("Tauri") && !out.contains("tauri."));
    assert!(p.lexicon().get("TAURI").is_some());
    assert!(p.set_lexicon_entry("GPT", "gpt 5", Origin::User).is_err());
    assert!(p.set_lexicon_entry("", "x", Origin::User).is_err());
    p.remove_lexicon_entry("tauri")
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(p.lexicon().get("tauri").is_none());
    assert!(matches!(
        p.remove_lexicon_entry("tauri"),
        Err(PersonaError::NotInLexicon { .. })
    ));
}

/// Tekst bez liczb, skrótów, URL-i i kodu przechodzi bez zmian; cyfry nigdy nie zostają.
pub fn normalizer_basic_invariants<P: Persona>(p: &P) {
    for plain in [
        "Dzień dobry, jak się masz?",
        "Zażółć gęślą jaźń.",
        "Mixed PL i EN: deploy na staging, potem review.",
    ] {
        assert_eq!(p.normalize_pl(plain), plain);
    }
    for with_digits in ["Mam 3 koty.", "Rok 2026 i 14:05.", "Kod 1234567890123456."] {
        let out = p.normalize_pl(with_digits);
        assert!(!has_ascii_digit(&out), "cyfry w wyniku: {out}");
    }
}

/// Plan: kod i tabele na ekran, kanał mówiony bez markdown, znaczników i cyfr.
pub fn plan_separates_channels<P: Persona>(p: &P) {
    let text = "[emocja:radość] **Gotowe!** Zrobiłam 3 poprawki.\n\n```rust\nfn main() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nTo wszystko.";
    let table = EngineStyleTable::neutral(EngineKind::Generic);
    let plan = p
        .plan(&PersonaId::alfa(), text, &table)
        .unwrap_or_else(|e| panic!("{e}"));
    let spoken = plan.spoken_text();
    for forbidden in ["```", "**", "[emocja", "fn main", "|"] {
        assert!(
            !spoken.contains(forbidden),
            "{forbidden} w kanale mówionym: {spoken}"
        );
    }
    assert!(
        !has_ascii_digit(&spoken),
        "cyfry w kanale mówionym: {spoken}"
    );
    assert!(plan.sentences.iter().all(|s| !s.text.trim().is_empty()));
    assert!(plan.on_screen.iter().any(|b| b.contains("fn main")));
    assert!(plan.on_screen.iter().any(|b| b.contains("| a | b |")));
    assert!(spoken.contains("Gotowe") && spoken.contains("To wszystko"));
    assert!(
        p.plan(
            &PersonaId::new("omega").unwrap_or_else(|e| panic!("{e}")),
            "x",
            &table
        )
        .is_err()
    );
    assert!(!CODE_ON_SCREEN.chars().any(|c| c.is_ascii_digit()));
}

fn run_chunker(chunker: &mut dyn SpeechChunker, pieces: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for piece in pieces {
        out.extend(chunker.push(piece).into_iter().map(|c| c.text));
    }
    out.extend(chunker.finish().into_iter().map(|c| c.text));
    out
}

/// Chunker zachowuje treść, nie tnie po skrócie „np.” i daje ten sam wynik w strumieniu.
pub fn chunker_preserves_text<P: Persona>(p: &P) {
    let text = "Na przykład np. to działa. Kosztuje 3.5 tys. Drugie zdanie!\nTrzecie; czwarte";
    let whole = run_chunker(p.chunker(ChunkerCfg::default()).as_mut(), &[text]);
    assert_eq!(collapse_ws(&whole.join(" ")), collapse_ws(text));
    assert!(whole.iter().all(|c| !c.trim().is_empty()));
    assert!(whole.iter().all(|c| !c.ends_with("np.")), "{whole:?}");
    let pieces: Vec<String> = text.chars().map(String::from).collect();
    let refs: Vec<&str> = pieces.iter().map(String::as_str).collect();
    let streamed = run_chunker(p.chunker(ChunkerCfg::default()).as_mut(), &refs);
    assert_eq!(streamed, whole);
    let mut empty = p.chunker(ChunkerCfg::default());
    assert!(empty.push("").is_empty());
    let rest = empty.finish();
    assert!(rest.is_empty() || rest.iter().all(|c| c.boundary == Boundary::End));
}

/// Uruchamia cały zestaw; `factory` daje świeżą instancję.
pub fn run_all<P, F>(factory: F)
where
    P: Persona,
    F: Fn() -> P,
{
    builtin_bibles(&factory());
    lexicon_priority_and_validation(&factory());
    normalizer_basic_invariants(&factory());
    plan_separates_channels(&factory());
    chunker_preserves_text(&factory());
}
