use platform_contract::WindowId;

use crate::numbers::{parse_prefix, unambiguous};
use crate::*;

const CASES: &str = include_str!("../../../evals/F5/voice/dictation-cases.json");

#[test]
fn normalization_cases_from_evals() {
    let doc: serde_json::Value = serde_json::from_str(CASES).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    assert!(cases.len() >= 28);
    let mut failed = Vec::new();
    for c in cases {
        let mut ctx = TextContext::start(true);
        let got = normalize(c["input"].as_str().unwrap(), &mut ctx);
        if got != c["expected"].as_str().unwrap() {
            failed.push(format!("{}: {:?} ≠ {:?}", c["id"], got, c["expected"]));
        }
    }
    eprintln!(
        "F5-10 (normalizacja, CI): {}/{}",
        cases.len() - failed.len(),
        cases.len()
    );
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn numbers_grammar() {
    let p = |s: &str| parse_prefix(&s.split(' ').collect::<Vec<_>>());
    assert_eq!(p("dwadziescia trzy"), Some((23, 2)));
    assert_eq!(p("dwa tysiace dwadziescia szesc lat"), Some((2026, 4)));
    assert_eq!(p("piec milionow trzysta tysiecy"), Some((5_300_000, 4)));
    assert_eq!(p("sto dwadziescia"), Some((120, 2)));
    assert_eq!(p("dwa trzy"), Some((2, 1)));
    assert_eq!(p("dwadziescia sto"), Some((20, 1)));
    assert_eq!(p("tysiac tysiac"), Some((1_000, 1)));
    assert_eq!(p("tysiac milion"), Some((1_000, 1)));
    assert_eq!(p("zero tysiecy"), Some((0, 1)));
    assert_eq!(p("dwadziescia zero"), Some((20, 1)));
    assert_eq!(p("kot"), None);
    assert!(unambiguous(23, 2) && unambiguous(15, 1));
    assert!(!unambiguous(5, 1) && !unambiguous(100, 1) && !unambiguous(1_000, 1));
}

#[test]
fn context_spacing_across_phrases_and_controls() {
    let mut ctx = TextContext::start(true);
    let a = normalize("dzień dobry przecinek", &mut ctx);
    let b = normalize("jak się masz znak zapytania", &mut ctx);
    let c = normalize("dobrze", &mut ctx);
    assert_eq!(format!("{a}{b}{c}"), "Dzień dobry, jak się masz? Dobrze");
    assert_eq!(control_command("Cofnij to."), Some(ControlCommand::Undo));
    assert_eq!(
        control_command("koniec dyktowania!"),
        Some(ControlCommand::Stop)
    );
    assert_eq!(control_command("cofnij to zdanie"), None);
    assert!(is_terminal_image(
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
    ));
    assert!(!is_terminal_image("notepad.exe"));
}

fn target(terminal: bool) -> DictationTarget {
    DictationTarget {
        window: WindowId(7),
        pid: 70,
        app: "notepad.exe".into(),
        terminal,
    }
}

fn texts(actions: &[DictationAction]) -> Vec<String> {
    actions
        .iter()
        .map(|a| match a {
            DictationAction::Type { text, .. } => text.clone(),
            DictationAction::Erase { chars, .. } => format!("⌫{chars}"),
        })
        .collect()
}

#[test]
fn machine_pause_resume_undo_and_terminal() {
    let mut m = DictationMachine::new(DictationCfg::default());
    assert_eq!(m.final_text("x", 0), Err(DictationError::NotActive));
    m.start(target(false), DictationMode::Toggle);
    let a = m.final_text("ala ma kota kropka", 100).unwrap();
    assert_eq!(texts(&a), vec!["Ala ma kota.".to_owned()]);
    m.typed(1, 12, 150);
    assert_eq!(m.status().typed_chars, 12);
    // Zmiana okna → pauza; fraza czeka; powrót → wpisanie.
    assert!(m.foreground(Some(WindowId(9))).is_empty());
    assert_eq!(
        m.status().phase,
        DictationPhase::Paused(PauseReason::FocusChanged)
    );
    assert!(m.final_text("drugie zdanie", 200).unwrap().is_empty());
    assert!(m.status().pending_chars > 0);
    let resumed = m.foreground(Some(WindowId(7)));
    assert_eq!(texts(&resumed), vec![" Drugie zdanie".to_owned()]);
    // Częściowe wpisanie: reszta czeka.
    m.typed(2, 4, 300);
    assert_eq!(
        texts(&m.foreground(Some(WindowId(7)))),
        vec!["gie zdanie".to_owned()]
    );
    m.typed(2, 10, 320);
    // „cofnij to” → Backspace × (4 + 10).
    let undo = m.final_text("cofnij to", 400).unwrap();
    assert_eq!(texts(&undo), vec!["⌫14".to_owned()]);
    m.erased(2, 14);
    let again = m.final_text("trzecie", 500).unwrap();
    assert_eq!(
        texts(&again),
        vec![" Trzecie".to_owned()],
        "kontekst sprzed cofniętej frazy"
    );
    // Cofnięcie frazy niewpisanej — tylko usunięcie z kolejki.
    let undo2 = m.final_text("cofnij to", 510).unwrap();
    assert!(undo2.is_empty());
    assert_eq!(m.status().pending_chars, 0);
    // Okno czasu „cofnij to”.
    m.final_text("czwarte", 600).unwrap();
    m.typed(4, 8, 600);
    assert!(m.final_text("cofnij to", 600 + 31_000).unwrap().is_empty());
    let events = m.take_events();
    assert!(events.contains(&DictationEvent::UndoUnavailable));
    assert!(events.contains(&DictationEvent::Resumed));
    // Za długa fraza.
    let long = "a ".repeat(1_500);
    assert!(matches!(
        m.final_text(&long, 700),
        Err(DictationError::Rejected(_))
    ));
    // Terminal: Enter zablokowany.
    m.start(target(true), DictationMode::PushToTalk);
    let t = m.final_text("dir nowa linia", 800).unwrap();
    assert_eq!(texts(&t), vec!["Dir ".to_owned()]);
    assert!(m.take_events().contains(&DictationEvent::NewlineBlocked));
    // „koniec dyktowania” porzuca kolejkę.
    m.final_text("koniec dyktowania", 900).unwrap();
    assert_eq!(m.status().phase, DictationPhase::Idle);
    assert!(matches!(
        m.take_events().last(),
        Some(DictationEvent::Stopped {
            reason: StopReason::Voice,
            dropped_chars: 4
        })
    ));
}

#[test]
fn user_busy_and_events() {
    let mut m = DictationMachine::new(DictationCfg::default());
    m.start(target(false), DictationMode::Toggle);
    m.final_text("test", 0).unwrap();
    m.user_busy();
    assert_eq!(
        m.status().phase,
        DictationPhase::Paused(PauseReason::UserTyping)
    );
    assert!(
        m.foreground(Some(WindowId(7))).is_empty(),
        "pauza użytkownika trwa"
    );
    assert_eq!(texts(&m.resume_after_user()), vec!["Test".to_owned()]);
    m.refuse(RefuseReason::PasswordField);
    for e in m.take_events() {
        let bus = e.to_bus_event();
        assert_eq!(bus.kind.as_str(), e.name());
        assert!(
            !bus.payload.to_string().contains("Test"),
            "zdarzenia bez treści"
        );
    }
    assert!(event_schema().is_object());
    assert_eq!(m.target().map(|t| t.pid), Some(70));
    assert!(m.cfg().block_enter_in_terminals);
}
