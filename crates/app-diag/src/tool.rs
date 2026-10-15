//! Narzędzie agentek `system_diagnostics`: raport stanu komputera (zapis parametrów z oceną:
//! kondycja dysków, miejsce, zdarzenia awarii, kody niebieskich ekranów, temperatury, bateria)
//! i — na prośbę — lekkie testy (procesor, pamięć, sieć, spójność). Testów dysków agentka nie
//! uruchamia (zapis setek MB na dysk użytkownika — tylko z Ustawień). Przez Brokera jak odczyt
//! systemu (`gui.control(system-info.exe)`), wynik niezaufany (treść dziennika systemu).

use std::sync::Arc;

use async_trait::async_trait;
use pc_diag_contract::{CheckKind, rating, report};
use risk_classifier_contract::Reversibility;
use safety_broker_contract::{AppSelector, Capability, TaintSource};
use serde::Deserialize;
use serde_json::{Value, json};
use tools_common_contract::text::truncate_chars;
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, action_request,
    base_facts, parse_args, report_untrusted,
};

use crate::DiagApp;

/// Pseudo-aplikacja odczytu systemu (ta sama co `tools-system`).
const SYSINFO_APP: &str = "system-info.exe";
/// Najdłuższy tekst wyniku.
const OUTPUT_MAX: usize = 30_000;
/// Testy dozwolone agentce.
const AGENT_TESTS: [&str; 5] = ["integrity", "cpu_single", "cpu_multi", "memory", "network"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default)]
    refresh: Option<bool>,
    #[serde(default)]
    tests: Vec<String>,
}

/// `system_diagnostics`.
pub struct DiagTool {
    app: Arc<DiagApp>,
    gate: BrokerGate,
    manifest: ToolManifest,
}

impl DiagTool {
    /// Narzędzie nad diagnostyką aplikacji.
    pub fn new(app: Arc<DiagApp>, gate: BrokerGate) -> Self {
        Self {
            app,
            gate,
            manifest: ToolManifest {
                name: "system_diagnostics".into(),
                id: "app-diag.report".into(),
                title: "Diagnostyka komputera".into(),
                description: "Raport stanu komputera: procesor, pamięć, dyski (kondycja, wolne miejsce, błędy w dzienniku przypisane do dysku), karty graficzne ze sterownikami, sieć, bateria, temperatury, niebieskie ekrany z objaśnieniem kodu i nieoczekiwane wyłączenia z ostatnich 7 dni, z oceną. `refresh` — nowy zapis parametrów (kilka sekund). `tests` — lekkie testy: integrity, cpu_single, cpu_multi, memory, network (testy dysków uruchamia tylko użytkownik w Ustawieniach → Diagnostyka komputera). Treść dziennika systemu jest niezaufana.".into(),
                input_schema: json!({"type": "object", "additionalProperties": false,
                    "properties": {
                        "refresh": {"type": "boolean", "description": "Zrób nowy zapis parametrów."},
                        "tests": {"type": "array", "items": {"type": "string", "enum": AGENT_TESTS},
                                  "description": "Lekkie testy do uruchomienia."}}}),
                output_schema: json!({"type": "object"}),
                reversible: Reversibility::Yes,
                capabilities: vec!["gui.control".into()],
                groups: vec!["system".into(), "system.read".into()],
                mutating: false,
                untrusted_output: Some(TaintSource::File),
            },
        }
    }
}

#[async_trait]
impl Tool for DiagTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: Value, ctx: &ToolCtx) -> ToolOutcome {
        let a: Args = match parse_args(args) {
            Ok(a) => a,
            Err(e) => return *e,
        };
        if let Some(bad) = a.tests.iter().find(|t| !AGENT_TESTS.contains(&t.as_str())) {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!(
                    "Test `{bad}` jest niedostępny dla agentki (dozwolone: {}).",
                    AGENT_TESTS.join(", ")
                ),
            );
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let action = "diagnostyka komputera";
        let cap = match AppSelector::parse(SYSINFO_APP) {
            Ok(app) => Capability::GuiControl(app),
            Err(e) => return ToolOutcome::failed(ToolErrorKind::Internal, e.to_string()),
        };
        let mut facts = base_facts(&self.manifest, ctx);
        facts.touches_private_data = true;
        let auth = match self
            .gate
            .authorize(action_request(ctx, cap.clone(), facts), ctx)
            .await
        {
            Ok(a) => a,
            Err(e) => return e.into_outcome(action),
        };
        if let Err(e) = self.gate.verify(&auth, &cap, &ctx.holder) {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return e.into_outcome(action);
        }
        let service = self.app.service();
        let snapshot = match (a.refresh.unwrap_or(false), service.latest()) {
            (false, Some(s)) => Ok(s),
            _ => service.record().await,
        };
        let run = if a.tests.is_empty() {
            None
        } else {
            let kinds = a
                .tests
                .iter()
                .filter_map(|t| CheckKind::from_key(t))
                .collect();
            Some(service.run(kinds, None).await)
        };
        self.gate.release(std::slice::from_ref(&auth)).await;
        let snapshot = match snapshot {
            Ok(s) => s,
            Err(e) => {
                return ToolOutcome::failed(
                    ToolErrorKind::Io,
                    format!("Nie odczytano parametrów: {e}."),
                );
            }
        };
        let run = match run.transpose() {
            Ok(r) => r,
            Err(e) => return ToolOutcome::failed(ToolErrorKind::Io, format!("Testy: {e}.")),
        };
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let assessment = rating::integrity(&snapshot);
        let text = format!(
            "Ocena: {}\n{}\n\n{}",
            report::verdict_word(assessment.verdict),
            assessment.notes.join("\n"),
            report::markdown(&snapshot, run.as_ref())
        );
        let (text, cut) = truncate_chars(&text, OUTPUT_MAX);
        let data = json!({
            "verdict": assessment.verdict,
            "notes": assessment.notes,
            "at": snapshot.at,
            "run": run.as_ref().map(|r| json!({"overall": r.overall, "results": r.results})),
        });
        let mut out = ToolOutcome::ok(text, data).untrusted(TaintSource::File);
        out.truncated = cut;
        out.approval = auth.approval;
        out
    }
}
