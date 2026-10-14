use super::*;
use crate::message::ImageSource;

fn level(text: &str) -> Difficulty {
    estimate_text(text, false, false).level(Strategy::Balanced)
}

#[test]
fn everyday_requests_are_easy() {
    for text in [
        "Cześć!",
        "Która jest godzina?",
        "Dzięki, to wszystko.",
        "Jaka będzie jutro pogoda w Krakowie?",
        "Przetłumacz na angielski: dzień dobry",
        "Naprawdę? To ciekawe.",
        "What's the capital of France?",
    ] {
        assert_eq!(level(text), Difficulty::Easy, "{text}");
    }
}

#[test]
fn work_requests_are_medium() {
    for text in [
        "Wyjaśnij, jak działa fotosynteza.",
        "Wyjasnij mi roznice miedzy TCP i UDP",
        "Popraw ten kod:\n```rust\nfn main() { println!(\"hi\") }\n```",
        "Napraw błąd w funkcji logowania",
        "Explain how a hash map works.",
        "Najpierw sprawdź pocztę, a potem przygotuj listę spraw na jutro i wyślij ją do mnie.",
    ] {
        assert_eq!(level(text), Difficulty::Medium, "{text}");
    }
}

#[test]
fn design_and_reasoning_with_code_or_steps_are_hard() {
    let hard = [
        "Przeanalizuj ten kod i zaproponuj refaktoryzację krok po kroku:\n```ts\nasync function load() {\n  const r = await fetch(url);\n  return r.json();\n}\n```",
        "Zaprojektuj architekturę usługi kolejkowej: najpierw opisz wymagania, następnie model danych, a potem plan wdrożenia. Uzasadnij wybory.",
        "Design an algorithm for scheduling meetings across time zones, prove it is correct and then optimise it step by step.",
    ];
    for text in hard {
        assert_eq!(level(text), Difficulty::Hard, "{text}");
    }
}

#[test]
fn polish_words_match_without_diacritics_and_by_word_start() {
    let with = estimate_text("Przeanalizuj wyniki", false, false);
    let without = estimate_text("przeanalizujesz WYNIKI", false, false);
    assert!(with.signals.contains(&Signal::Reasoning));
    assert!(without.signals.contains(&Signal::Reasoning));
    assert!(
        !estimate_text("To naprawdę dobre", false, false)
            .signals
            .contains(&Signal::Reasoning),
        "„naprawdę” to nie prośba o naprawę"
    );
}

#[test]
fn strategy_moves_thresholds() {
    let e = estimate_text("Wyjaśnij, jak działa fotosynteza.", false, false);
    assert_eq!(e.level(Strategy::Economy), Difficulty::Easy);
    assert_eq!(e.level(Strategy::Balanced), Difficulty::Medium);
    let code = estimate_text(
        "Napisz funkcję, która sortuje listę:\n```py\ndef sort(xs):\n    pass\n```",
        false,
        false,
    );
    assert_eq!(code.level(Strategy::Economy), Difficulty::Easy);
    assert_eq!(code.level(Strategy::Quality), Difficulty::Medium);
    for score in 0..=100u8 {
        // Wyższa strategia nigdy nie daje niższego poziomu.
        assert!(Strategy::Economy.level(score) <= Strategy::Balanced.level(score));
        assert!(Strategy::Balanced.level(score) <= Strategy::Quality.level(score));
    }
}

#[test]
fn image_long_text_and_questions_count() {
    let image = estimate_text("Co jest na tym zdjęciu?", true, false);
    assert!(image.signals.contains(&Signal::Image));
    assert_eq!(image.level(Strategy::Balanced), Difficulty::Medium);
    let long = "To jest długi opis sprawy. ".repeat(30);
    let e = estimate_text(&long, false, false);
    assert!(e.signals.contains(&Signal::Long));
    assert_eq!(
        e.level(Strategy::Balanced),
        Difficulty::Easy,
        "sama długość nie robi zadania trudnym"
    );
    let q = estimate_text("Ile to kosztuje? A ile trwa dostawa?", false, false);
    assert!(q.signals.contains(&Signal::Questions));
    assert_eq!(estimate_text("", false, false).score, 0);
    assert_eq!(
        estimate_text("", false, false).reasons(),
        "krótka, prosta prośba"
    );
}

#[test]
fn short_follow_up_in_code_conversation_is_not_easy() {
    let alone = estimate_text("popraw to", false, false);
    let in_code = estimate_text("popraw to", false, true);
    assert_eq!(alone.level(Strategy::Balanced), Difficulty::Easy);
    assert!(in_code.signals.contains(&Signal::CodeContext));
    assert_eq!(in_code.level(Strategy::Balanced), Difficulty::Easy.up());
    assert_eq!(in_code.reasons(), "rozmowa o kodzie");
}

#[test]
fn request_uses_last_user_text_and_code_from_history() {
    let mut req = ChatRequest::new(
        "auto",
        vec![
            Message::user_text("Napisz parser:\n```rust\nfn parse() {}\n```"),
            Message::assistant_text("```rust\nfn parse(s: &str) {}\n```"),
            Message::user_text("dodaj obsługę błędów"),
        ],
    );
    let e = estimate_request(&req);
    assert!(e.signals.contains(&Signal::CodeContext), "{e:?}");
    req.messages.push(Message {
        role: Role::User,
        content: vec![ContentBlock::Image {
            source: ImageSource::Url {
                url: "https://example.invalid/a.png".into(),
            },
        }],
        interruption: None,
    });
    // Sam obraz bez tekstu — liczy się ostatnia wiadomość z tekstem.
    assert!(
        estimate_request(&req)
            .signals
            .contains(&Signal::CodeContext)
    );
    let empty = ChatRequest::new("auto", Vec::new());
    assert_eq!(estimate_request(&empty).score, 0);
}

#[test]
fn score_is_bounded_and_deterministic() {
    let text = format!(
        "{}\n1. przeanalizuj\n2. zaprojektuj\n```\ncode\n```\n?? ??",
        "x".repeat(2_000)
    );
    let a = estimate_text(&text, true, true);
    assert!(a.score <= 100);
    assert_eq!(a, estimate_text(&text, true, true));
}
