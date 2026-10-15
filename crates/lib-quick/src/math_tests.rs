//! Testy rachunków (`math.rs`).
use super::answer;

#[test]
fn computes_and_explains() {
    assert_eq!(
        answer("ile to jest 7 razy 8").as_deref(),
        Some("7 × 8 = **56**")
    );
    assert_eq!(answer("2+2*2").as_deref(), Some("2 + 2 × 2 = **6**"));
    assert_eq!(
        answer("oblicz (2+3)*4").as_deref(),
        Some("(2 + 3) × 4 = **20**")
    );
    assert_eq!(answer("20% z 150").as_deref(), Some("20% z 150 = **30**"));
    assert_eq!(answer("pierwiastek z 81").as_deref(), Some("√81 = **9**"));
    assert_eq!(
        answer("dwa do potegi dziesiec").as_deref(),
        Some("2 ^ 10 = **1024**")
    );
    assert_eq!(
        answer("ile to 3,5 razy 2").as_deref(),
        Some("3,5 × 2 = **7**")
    );
    assert_eq!(
        answer("12 do kwadratu").as_deref(),
        Some("12 ^ 2 = **144**")
    );
    let div = answer("ile to jest 7 podzielone przez 2").unwrap_or_default();
    assert!(
        div.starts_with("7 : 2 = **3,5**") && div.contains("3 reszty 1"),
        "{div}"
    );
    assert!(answer("5/0").unwrap_or_default().contains("zero"));
    assert!(answer("10^40").unwrap_or_default().contains("za duży"));
    assert_eq!(answer("100 plus 23%").as_deref(), Some("100 + 23% = **123**"));
    assert_eq!(answer("200 - 10%").as_deref(), Some("200 − 10% = **180**"));
    assert_eq!(answer("20% z 150").as_deref(), Some("20% z 150 = **30**"));
    assert!(answer("(0-8)^0,5").unwrap_or_default().contains("liczb rzeczywistych"));
    assert_eq!(answer("12345678901234567890 minus 1"), None, "poza precyzją");
}

#[test]
fn leaves_other_questions_to_the_model() {
    for text in [
        "ile kosztuje 5 jablek",
        "ile to jest kilometr",
        "2026-10-15",
        "7",
        "oblicz",
        "ile lat ma ziemia",
        "2 +",
        "(2+3",
    ] {
        assert_eq!(answer(text), None, "{text}");
    }
}
