use super::*;
use serde_json::json;

fn lib() -> Vec<PipelineDef> {
    pipelines::library(&[])
}

#[test]
fn named_pipeline_with_positional_models() {
    let p = plan(
            &json!({"pipeline": "UI-Design", "input": "x", "models": ["openai:astra", "anthropic:opus", "openai:sol"]}),
            &lib(),
        )
        .unwrap();
    assert_eq!(p.len(), 3);
    assert_eq!(p[0].model.as_deref(), Some("openai:astra"));
    assert_eq!(p[2].role, "coder");
    assert_eq!(p[2].title, "Kod React/Tailwind");
}

#[test]
fn custom_steps_validate_models_and_goals() {
    assert!(plan(&json!({"steps": [{"role": "coder"}]}), &lib()).is_err());
    assert!(
        plan(
            &json!({"steps": [{"role": "coder", "goal": "g", "model": "bez-dostawcy"}]}),
            &lib()
        )
        .is_err()
    );
    let ok = plan(
        &json!({"steps": [{"role": "coder", "goal": "g", "model": "auto"}]}),
        &lib(),
    )
    .unwrap();
    assert_eq!(ok[0].model, None);
    assert!(plan(&json!({}), &lib()).is_err());
    assert!(plan(&json!({"pipeline": "nieznany"}), &lib()).is_err());
}

#[test]
fn render_passes_previous_output() {
    assert_eq!(
        render("a {{prev}} b {{input}}", "IN", Some("PREV")),
        "a PREV b IN"
    );
    assert_eq!(render("zrób", "IN", None), "zrób\n\nWejście:\nIN");
    assert_eq!(
        render("zrób", "IN", Some("P")),
        "zrób\n\nWynik poprzedniego etapu:\nP"
    );
    // Etap po pierwszym z samym {{input}} nie gubi wyniku poprzedniego etapu.
    assert_eq!(
        render("zrób {{input}}", "IN", Some("P")),
        "zrób IN\n\nWynik poprzedniego etapu:\nP"
    );
    assert_eq!(render("zrób {{input}}", "IN", None), "zrób IN");
    // Znacznik w treści wejścia to tekst, nie miejsce na wynik etapu.
    assert_eq!(
        render("{{input}} / {{prev}}", "x {{prev}}", Some("P")),
        "x {{prev}} / P"
    );
    assert_eq!(body_of("[Delta (Koderka)]: kod"), "kod");
}
