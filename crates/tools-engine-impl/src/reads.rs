//! Narzędzia odczytu: stan instalacji, dokumentacja silnika, odczyt sceny i strumieniowanie terenu.

use std::time::{Duration, Instant};

use serde_json::json;
use tools_common_contract::text::truncate_chars;
use tools_common_contract::{ToolCtx, ToolOutcome};
use tools_engine_contract::{DocsArgs, EngineMethod, InspectArgs, InspectWhat, StreamArgs};

use crate::Core;
use crate::gate::{Step, engine_fail};
use crate::tools::{TEXT_MAX, pretty};

/// Najdłużej `engine_stream` z `until_ready`.
const STREAM_LIMIT: Duration = Duration::from_secs(30);

impl Core {
    pub(crate) fn status(&self) -> ToolOutcome {
        let install = self.port.installation();
        let state = self.port.state();
        let text = match (&install.root, &install.server, state.running) {
            (None, _, _) => "Nie wskazano katalogu silnika (Ustawienia → Zaawansowane → Katalog silnika gier NextGO.Gray).".to_owned(),
            (Some(root), None, _) => format!(
                "Silnik w {} nie jest zbudowany (brak target/release/engine-server.exe).",
                root.display()
            ),
            (Some(_), Some(_), false) => format!(
                "Silnik {} gotowy, serwer nie działa (sprawdzenie: {}). Uruchom: engine_start.",
                install.version.as_deref().unwrap_or("?"),
                install.verification.as_deref().unwrap_or("brak")
            ),
            (Some(_), Some(_), true) => format!(
                "Serwer silnika działa: {} ({}, scena {}).",
                state.url.as_deref().unwrap_or("?"),
                state.mode.as_deref().unwrap_or("?"),
                state.scene.as_deref().unwrap_or("?")
            ),
        };
        ToolOutcome::ok(text, json!({"installation": install, "state": state}))
    }

    pub(crate) async fn docs(&self, a: DocsArgs) -> Step<ToolOutcome> {
        let Some(file) = a.file else {
            let list = self.port.docs().await.map_err(|e| engine_fail(&e))?;
            let lines: Vec<_> = list
                .iter()
                .map(|d| format!("- {} — {} ({} B)", d.name, d.title, d.bytes))
                .collect();
            let text = format!(
                "Dokumentacja silnika ({} plików):\n{}",
                list.len(),
                lines.join("\n")
            );
            return Ok(ToolOutcome::ok(text, json!({"files": list})));
        };
        let content = self
            .port
            .read_doc(&file)
            .await
            .map_err(|e| engine_fail(&e))?;
        let total = content.chars().count();
        let offset = a.offset.unwrap_or(0) as usize;
        let rest: String = content.chars().skip(offset).collect();
        let (part, truncated) = truncate_chars(&rest, TEXT_MAX);
        let next = truncated.then(|| offset + part.chars().count());
        let mut out = ToolOutcome::ok(
            match next {
                Some(n) => format!("{part}\n\n[ciąg dalszy: engine_docs z offset {n} z {total}]"),
                None => part.to_owned(),
            },
            json!({"file": file, "chars": total, "next_offset": next}),
        );
        out.truncated = truncated;
        Ok(out)
    }

    pub(crate) async fn inspect(&self, a: InspectArgs) -> Step<ToolOutcome> {
        let (method, params) = match a.what.unwrap_or_default() {
            InspectWhat::Streaming => (EngineMethod::InspectWorldStreaming, json!({})),
            InspectWhat::Selection => (EngineMethod::GetSpatialSelection, json!({})),
            InspectWhat::Scene => {
                let mut p = json!({});
                if let Some(r) = a.radius_meters {
                    p["radius_meters"] = json!(r);
                }
                if let Some(c) = a.center {
                    p["center"] = json!(c);
                }
                if let Some(l) = a.limit {
                    p["limit"] = json!(l);
                }
                (EngineMethod::InspectSceneTelemetry, p)
            }
        };
        let r = self.rpc(method, params).await?;
        let (text, truncated) = pretty(&r);
        let mut out = ToolOutcome::ok(text, r);
        out.truncated = truncated;
        Ok(out)
    }

    pub(crate) async fn stream(&self, a: StreamArgs, ctx: &ToolCtx) -> Step<ToolOutcome> {
        let params = json!({"budget": a.budget.unwrap_or(32)});
        let until_ready = a.until_ready.unwrap_or(true);
        let started = Instant::now();
        let mut rounds = 0u32;
        loop {
            let r = self
                .cancellable(ctx, EngineMethod::StreamWorld, params.clone())
                .await?;
            rounds += 1;
            let ready = r["near_ready"].as_bool().unwrap_or(false);
            let enabled = r["enabled"].as_bool().unwrap_or(false);
            if !until_ready || ready || !enabled || started.elapsed() > STREAM_LIMIT {
                let text = match (enabled, ready) {
                    (false, _) => {
                        "Strumieniowanie terenu jest wyłączone (world.configure).".to_owned()
                    }
                    (true, true) => format!(
                        "Teren gotowy po {rounds} wywołaniach: {} fragmentów.",
                        r["resident_chunks"]
                    ),
                    (true, false) => format!(
                        "Teren jeszcze niegotowy po {rounds} wywołaniach — wywołaj ponownie."
                    ),
                };
                return Ok(ToolOutcome::ok(text, r));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
