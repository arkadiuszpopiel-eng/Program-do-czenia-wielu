use super::*;

fn r(provider: &str, model: &str, rung: Rung, local: bool) -> Ranked {
    Ranked {
        candidate: Candidate::new(provider, model),
        rung,
        local,
    }
}

fn names(list: &[Candidate]) -> Vec<String> {
    list.iter().map(|c| c.model.clone()).collect()
}

#[test]
fn rung_from_model_name() {
    for (model, rung) in [
        ("bielik-4.5b-v3.0-instruct-q8_0", Rung::Light),
        ("qwen3-27b-instruct-q4_k_m", Rung::Standard),
        ("llama-3.1-405b", Rung::Strong),
        ("claude-haiku-5-5", Rung::Light),
        ("gemini-2.5-flash-lite", Rung::Light),
        ("gpt-6-luna", Rung::Standard),
        ("claude-sonnet-5-5", Rung::Standard),
        ("deepseek-chat", Rung::Standard),
        ("gpt-6-sol", Rung::Strong),
        ("claude-opus-5-5", Rung::Strong),
        ("claude-fable-5-1", Rung::Strong),
        ("gemini-2.5-pro", Rung::Strong),
        ("openai/gpt-6-mini", Rung::Light),
    ] {
        assert_eq!(rung_by_name(model), rung, "{model}");
    }
    assert_eq!(params_billions("qwen3-30b-a3b"), Some(30.0));
    assert_eq!(params_billions("gpt-6-sol"), None);
    assert_eq!(params_billions("b"), None);
}

#[test]
fn difficulty_maps_to_rungs_and_class_floors() {
    assert_eq!(Rung::for_difficulty(Difficulty::Easy), Rung::Light);
    assert_eq!(Rung::for_difficulty(Difficulty::Hard), Rung::Strong);
    assert_eq!(Rung::Light.up(), Rung::Standard);
    assert_eq!(Rung::Strong.up(), Rung::Strong);
    assert_eq!(class_floor(TaskClass::Code), Difficulty::Medium);
    assert_eq!(class_floor(TaskClass::Conversation), Difficulty::Easy);
    assert_eq!(class_from_policy("code"), Some(TaskClass::Code));
    assert_eq!(class_from_policy("review"), Some(TaskClass::Planning));
    assert_eq!(class_from_policy("gui-vision"), Some(TaskClass::GuiVision));
    assert_eq!(class_from_policy("cokolwiek"), None);
}

#[test]
fn easy_starts_light_hard_starts_strong_rest_stays_as_fallback() {
    let list = || {
        vec![
            r("anthropic", "opus", Rung::Strong, false),
            r("openai", "luna", Rung::Standard, false),
            r("local", "qwen-27b", Rung::Standard, true),
            r("light", "bielik-4.5b", Rung::Light, true),
        ]
    };
    assert_eq!(
        names(&order_by_rung(list(), Rung::Light, true, None)),
        ["bielik-4.5b", "qwen-27b", "luna", "opus"]
    );
    assert_eq!(
        names(&order_by_rung(list(), Rung::Standard, true, None)),
        ["qwen-27b", "luna", "opus", "bielik-4.5b"]
    );
    assert_eq!(
        names(&order_by_rung(list(), Rung::Standard, false, None)),
        ["luna", "qwen-27b", "opus", "bielik-4.5b"],
        "bez „najpierw lokalny” zostaje kolejność kolejki"
    );
    assert_eq!(
        names(&order_by_rung(list(), Rung::Strong, true, None)),
        ["opus", "qwen-27b", "luna", "bielik-4.5b"]
    );
}

#[test]
fn rejected_model_goes_last() {
    let list = vec![
        r("anthropic", "opus", Rung::Strong, false),
        r("openai", "sol", Rung::Strong, false),
        r("openai", "luna", Rung::Standard, false),
    ];
    let opus = Candidate::new("anthropic", "opus");
    assert_eq!(
        names(&order_by_rung(list, Rung::Strong, true, Some(&opus))),
        ["sol", "luna", "opus"]
    );
}

#[test]
fn policy_defaults_and_partial_json() {
    let p = DifficultyPolicy::default();
    assert!(p.enabled && p.escalate && p.local_first);
    assert!(!p.reorder_queues, "ręczna kolejka ma pierwszeństwo");
    assert_eq!(p.review, Difficulty::Easy);
    let partial: DifficultyPolicy =
        serde_json::from_str(r#"{"strategy":"quality","rungs":{"openai:gpt-6-luna":"strong"}}"#)
            .unwrap();
    assert_eq!(partial.strategy, Strategy::Quality);
    assert!(partial.enabled, "brakujące pola — wartości domyślne");
    assert_eq!(partial.rungs["openai:gpt-6-luna"], Rung::Strong);
}

#[test]
fn note_summary_mentions_escalation() {
    let note = DifficultyNote {
        difficulty: Difficulty::Hard,
        score: Some(70),
        reasons: "kod, rozumowanie".into(),
        start: Rung::Strong,
        escalated_from: Some(Candidate::new("local", "qwen-27b")),
        kept_queue: false,
    };
    assert_eq!(
        note.summary(),
        "trudne (kod, rozumowanie) → szczebel mocny, podniesione po odrzuceniu odpowiedzi local:qwen-27b"
    );
    assert!(note.reorders());
    let kept = DifficultyNote {
        escalated_from: None,
        kept_queue: true,
        ..note
    };
    assert!(!kept.reorders());
    assert_eq!(
        kept.summary(),
        "trudne (kod, rozumowanie) → kolejka ustawiona ręcznie, bez zmian"
    );
}
