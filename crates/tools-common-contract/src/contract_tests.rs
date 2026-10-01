//! Współdzielone testy kontraktowe narzędzi (feature `contract-tests`): uruchamiane na każdym
//! narzędziu `-impl` i `-fake`. Sprawdzają manifest i to, że niepoprawne argumenty nigdy nie
//! kończą się wykonaniem.

use safety_broker_contract::Holder;

use crate::call::{Tool, ToolCtx, ToolErrorKind, ToolStatus};

/// Kontekst testowy (sesja `s1`, agentka `delta`, katalog roboczy `workdir`).
pub fn ctx(workdir: &str) -> ToolCtx {
    ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(workdir)
}

/// Manifest jest poprawny i zgodny z `ToolSpec` dla modelu.
pub fn manifest_is_valid<T: Tool + ?Sized>(tool: &T) {
    let m = tool.manifest();
    m.validate().unwrap_or_else(|e| panic!("{e}"));
    let spec = m.to_spec();
    assert_eq!(spec.name, m.name);
    assert_eq!(spec.input_schema, m.input_schema);
    assert!(
        !m.capabilities.is_empty() && !m.groups.is_empty(),
        "{}: zdolności i grupy ról",
        m.name
    );
}

/// Argumenty niebędące obiektem albo z nieznanym polem są odrzucane bez wykonania.
pub async fn invalid_args_rejected<T: Tool + ?Sized>(tool: &T, workdir: &str) {
    let c = ctx(workdir);
    for args in [
        serde_json::json!("x"),
        serde_json::json!([1, 2]),
        serde_json::json!({"__nieznane_pole__": true}),
    ] {
        let out = tool.call(args.clone(), &c).await;
        assert_eq!(
            out.status,
            ToolStatus::Failed {
                error: ToolErrorKind::InvalidArgs
            },
            "{}: {args}",
            tool.manifest().name
        );
        assert!(
            out.undo.is_none(),
            "{}: odrzucenie bez kroku cofania",
            tool.manifest().name
        );
    }
}

/// Anulowany kontekst nigdy nie kończy się sukcesem akcji zmieniającej stan.
pub async fn cancelled_ctx_does_not_mutate<T: Tool + ?Sized>(
    tool: &T,
    workdir: &str,
    valid_args: serde_json::Value,
) {
    let c = ctx(workdir);
    c.cancel.cancel();
    let out = tool.call(valid_args, &c).await;
    if tool.manifest().mutating {
        assert_ne!(out.status, ToolStatus::Ok, "{}", tool.manifest().name);
        assert!(out.undo.is_none());
    }
}

/// Cały zestaw dla jednego narzędzia.
pub async fn run_all<T: Tool + ?Sized>(tool: &T, workdir: &str, valid_args: serde_json::Value) {
    manifest_is_valid(tool);
    invalid_args_rejected(tool, workdir).await;
    cancelled_ctx_does_not_mutate(tool, workdir, valid_args).await;
}
