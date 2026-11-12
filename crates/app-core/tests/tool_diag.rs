//! Diagnostyka narzędzi agentek (docs/plan/45 §6.5): KAŻDE narzędzie rdzenia — także dodane
//! później — wywołane z POPRAWNYMI argumentami na rdzeniu, w którym każdy port maszyny jest
//! atrapą (`tool_rig/fakes.rs`), z poziomem L4 w profilu tymczasowym. Raport z wynikiem, czasem,
//! śladem na atrapach i zgodnością danych z `output_schema` trafia do `target/verify/narzedzia`.
//! Czerwone: odrzucenie poprawnych argumentów, błąd wewnętrzny, brak odpowiedzi, panika, pusty
//! tekst, dane niezgodne ze schematem, klasa wyniku inna niż we wzorcu
//! (`tests/tool_rig/wzorzec.json`) i narzędzie bez wpisu we wzorcu. Po świadomej zmianie
//! wzorzec odświeża `ALFA_TOOL_DIAG_UPDATE=1` (raport pokazuje, co się zmieniło).
//! katalog-przeglad: narzedzie

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod tool_rig;

use std::sync::Arc;
use std::time::{Duration, Instant};

use app_core::dto::SessionTemplate;
use safety_broker_contract::Holder;
use serde_json::Value;
use tool_rig::report::{self, Entry};
use tools_common_contract::{Tool, ToolCtx};

/// Najdłuższe dopuszczalne wywołanie na atrapach.
const LIMIT: Duration = Duration::from_secs(20);

/// Wywołuje narzędzie w osobnym zadaniu (panika nie przerywa przeglądu) i opisuje wynik;
/// `given` — argumenty próby straży zamiast przepisu i generatora.
async fn run(
    rig: &tool_rig::fakes::Rig,
    tool: Arc<dyn Tool>,
    given: Option<(Value, &'static str)>,
) -> Entry {
    let m = tool.manifest().clone();
    let (args, source) =
        given.unwrap_or_else(|| tool_rig::args::args_for(&m.name, &m.input_schema, &rig.targets));
    let args_error = report::check_output(&m.input_schema, &args);
    // Własna sesja każdego narzędzia: obca treść z jednego wywołania (oznaczenie sesji) nie
    // zmienia decyzji Brokera dla następnych; prośba o zatwierdzenie wygasa po 2 s. Narzędzia
    // przeglądarki dzielą sesję z `browser_open` (przeglądarka należy do sesji).
    // Wyszukanie otwiera własną stronę: osobna sesja zachowuje formularz innych prób.
    let session = if m.name.starts_with("browser_") && m.name != "browser_search" {
        rig.browser_session.clone()
    } else {
        rig.core
            .sessions_create(SessionTemplate::Empty)
            .await
            .unwrap()
            .id
    };
    let mut ctx = ToolCtx::new(Holder::agent(session.as_str(), "delta"))
        .with_workdir(&rig.workdir.to_string_lossy());
    ctx.approval_timeout = Duration::from_secs(2);
    let (runs, files) = (rig.exec.runs().len(), rig.fs.snapshot());
    let started = Instant::now();
    let call_args = args.clone();
    let task = tokio::spawn(async move { tool.call(call_args, &ctx).await });
    let (class, text, output_error) = match tokio::time::timeout(LIMIT, task).await {
        Err(_) => ("brak-odpowiedzi".into(), String::new(), None),
        Ok(Err(_)) => ("panika".into(), String::new(), None),
        Ok(Ok(out)) => (
            report::class(&out.status),
            out.text.clone(),
            report::check_output(&m.output_schema, &out.data),
        ),
    };
    let after = rig.fs.snapshot();
    let changed = after
        .iter()
        .filter(|(p, b)| files.get(*p) != Some(*b))
        .count()
        + files.keys().filter(|p| !after.contains_key(*p)).count();
    let exec = rig.exec.runs().len() - runs;
    let mut trace = Vec::new();
    if exec > 0 {
        trace.push(format!("exec {exec}"));
    }
    if changed > 0 {
        trace.push(format!("fs {changed}"));
    }
    Entry {
        name: m.name.clone(),
        id: m.id.clone(),
        schema: m.input_schema.clone(),
        groups: m
            .groups
            .iter()
            .map(|g| format!("{g:?}").to_lowercase())
            .collect(),
        mutating: m.mutating,
        source,
        args,
        class,
        ms: started.elapsed().as_millis(),
        text,
        output_error,
        args_error,
        trace: trace.join(", "),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_agent_tool_runs_with_valid_arguments_on_fakes() {
    let rig = tool_rig::fakes::rig().await;
    let mut tools = rig.core.agent_tools();
    assert!(tools.len() >= 60, "za mało narzędzi: {}", tools.len());
    tools.sort_by_key(|t| {
        let name = t.manifest().name.clone();
        (tool_rig::args::phase(&name), name)
    });

    let mut entries = Vec::new();
    for tool in &tools {
        entries.push(run(&rig, tool.clone(), None).await);
    }
    let update = std::env::var("ALFA_TOOL_DIAG_UPDATE").is_ok_and(|v| v == "1");
    let baseline = report::read_baseline();
    let mut problems = Vec::new();
    for e in &entries {
        for d in report::defects(e) {
            problems.push(format!("{}: {d}", e.name));
        }
        match baseline.get(&e.name) {
            _ if update => {}
            None => problems.push(format!(
                "{}: brak wpisu we wzorcu (wynik {}) — sprawdź raport i odśwież wzorzec",
                e.name, e.class
            )),
            Some(expected) if *expected != e.class => problems.push(format!(
                "{}: wynik {} zamiast {expected} ze wzorca",
                e.name, e.class
            )),
            Some(_) => {}
        }
    }
    // Próby straży: odmowa na poziomie L4 (pliki poświadczeń, zmienne Alfy, adresy lokalne).
    let secret = rig.workdir.join(".ssh").join("id_rsa");
    for (name, args, expected) in tool_rig::args::guard_probes(&secret.to_string_lossy()) {
        let Some(tool) = tools.iter().find(|t| t.manifest().name == name) else {
            problems.push(format!("{name}: próba straży bez narzędzia"));
            continue;
        };
        let mut e = run(&rig, tool.clone(), Some((args, "straż"))).await;
        if !e.class.starts_with(expected) {
            problems.push(format!(
                "{name}: próba straży {} zakończona {} zamiast {expected}",
                e.args, e.class
            ));
        }
        if e.text.trim().is_empty() {
            problems.push(format!("{name}: próba straży bez wyjaśnienia dla modelu"));
        }
        e.name = format!("{name} (straż)");
        entries.push(e);
    }
    if !update {
        for name in baseline.keys() {
            if !entries.iter().any(|e| &e.name == name) {
                problems.push(format!("{name}: wpis we wzorcu bez narzędzia — usuń go"));
            }
        }
    }
    report::write(&entries, &problems);
    let ocr_requests = rig.ocr.requests();
    assert_eq!(ocr_requests.len(), 1, "OCR używa wstrzykniętej atrapy");
    assert!(
        !ocr_requests[0].image.is_empty(),
        "atrapa dostała obraz zrzutu"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.name == "vision_ocr" && e.text.contains("Atrapa OCR"))
    );
    if update {
        report::write_baseline(&entries);
    }
    assert!(
        problems.is_empty(),
        "{} narzędzi, problemy ({}) — raport target/verify/narzedzia/raport.md:\n{}",
        entries.len(),
        problems.len(),
        problems.join("\n")
    );
}

/// Kontrola diagnostyki: każda usterka wyniku jest zgłaszana, a generator argumentów daje
/// wartości zgodne ze schematem — inaczej zielony raport niczego by nie dowodził.
#[test]
fn the_diagnostics_flag_every_defect_and_generate_valid_arguments() {
    let schema = serde_json::json!({
        "type": "object", "additionalProperties": false, "required": ["path", "count", "mode"],
        "properties": {
            "path": { "type": "string" },
            "count": { "type": "integer", "minimum": 2, "maximum": 5 },
            "mode": { "enum": ["szybko", "dokładnie"] },
            "extra": { "type": ["string", "null"] }
        }
    });
    let args = tool_rig::args::sample(&schema, "");
    assert_eq!(report::check_output(&schema, &args), None, "{args}");
    assert_eq!(args["count"], 2);
    let entry = |class: &str, text: &str| Entry {
        name: "atrapa".into(),
        id: "test.atrapa".into(),
        schema: schema.clone(),
        groups: vec![],
        mutating: false,
        source: "schemat",
        args: serde_json::json!({ "path": 1 }),
        class: class.into(),
        ms: 0,
        text: text.into(),
        output_error: report::check_output(&schema, &serde_json::json!({})),
        args_error: report::check_output(&schema, &serde_json::json!({ "path": 1 })),
        trace: String::new(),
    };
    for (class, expected) in [
        ("blad:invalid_args", "odrzucone jako błędne"),
        ("blad:internal", "błąd wewnętrzny"),
        ("brak-odpowiedzi", "brak-odpowiedzi"),
        ("panika", "panika"),
    ] {
        let found = report::defects(&entry(class, "tekst"));
        assert!(
            found.iter().any(|d| d.contains(expected)),
            "{class}: {found:?}"
        );
    }
    let found = report::defects(&entry("ok", " "));
    for expected in [
        "pusty tekst",
        "niezgodne z output_schema",
        "niezgodne z input_schema",
    ] {
        assert!(
            found.iter().any(|d| d.contains(expected)),
            "{expected}: {found:?}"
        );
    }
}
