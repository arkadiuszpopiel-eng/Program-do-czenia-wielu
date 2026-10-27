//! Wykonanie narzędzi: zgoda Brokera → port silnika → wynik dla modelu (tekst, dane, obraz).

use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tools_common_contract::text::truncate_chars;
use tools_common_contract::{
    ToolCtx, ToolErrorKind, ToolImage, ToolManifest, ToolOutcome, parse_args, report_untrusted,
};
use tools_engine_contract::{
    Capture, EngineMethod, HeightArgs, PreviewArgs, PreviewOptions, RaycastArgs, Scene, SelectArgs,
    ShaderArgs, StartArgs, StartOptions, StepArgs, TransactionArgs,
};

use crate::Core;
use crate::gate::{Step, engine_fail, fail};

/// Najdłuższy tekst dla modelu.
pub(crate) const TEXT_MAX: usize = 30_000;
/// Największy obraz zwracany modelowi.
const IMAGE_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Stały identyfikator transakcji z rozmowy, przebiegu i treści — bez numeru kroku: model, który
/// po upływie terminu ponawia to samo wywołanie w następnym kroku, trafia w ten sam identyfikator,
/// a silnik oddaje wynik pierwszego wykonania zamiast stosować zmianę drugi raz (przegląd
/// 2026-10-27). Celowe powtórzenie w tym samym przebiegu: nowa `expected_revision` albo własny id.
pub(crate) fn derived_id(ctx: &ToolCtx, kind: &str, body: &Value) -> String {
    let mut h = Sha256::new();
    h.update(ctx.holder.session.as_str().as_bytes());
    h.update(format!("{:?}|{kind}|", ctx.run).as_bytes());
    h.update(body.to_string().as_bytes());
    let hex: String = h.finalize()[..12]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("alfa-{hex}")
}

pub(crate) fn pretty(v: &Value) -> (String, bool) {
    let text = serde_json::to_string_pretty(v).unwrap_or_default();
    let (cut, truncated) = truncate_chars(&text, TEXT_MAX);
    (cut.to_owned(), truncated)
}

fn image(capture: &Capture) -> Step<ToolImage> {
    let meta = std::fs::metadata(&capture.png)
        .map_err(|e| fail(ToolErrorKind::Io, format!("Zrzut: {e}.")))?;
    if meta.len() > IMAGE_MAX_BYTES {
        return Err(fail(
            ToolErrorKind::Io,
            "Zrzut jest za duży — zmniejsz rozdzielczość.",
        ));
    }
    let bytes =
        std::fs::read(&capture.png).map_err(|e| fail(ToolErrorKind::Io, format!("Zrzut: {e}.")))?;
    Ok(ToolImage {
        media_type: "image/png".into(),
        data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

fn capture_outcome(capture: &Capture, what: &str) -> Step<ToolOutcome> {
    let rev = capture
        .revision
        .map_or_else(String::new, |r| format!(", rewizja {r}"));
    let mut out = ToolOutcome::ok(
        format!("{what}{rev}: {}", capture.png.display()),
        json!({"png": capture.png, "revision": capture.revision, "telemetry": capture.telemetry}),
    );
    out.images.push(image(capture)?);
    Ok(out)
}

impl Core {
    pub(crate) async fn rpc(&self, method: EngineMethod, params: Value) -> Step<Value> {
        self.port
            .call(method, params)
            .await
            .map_err(|e| engine_fail(&e))
    }

    pub(crate) async fn run(
        &self,
        m: &ToolManifest,
        args: Value,
        ctx: &ToolCtx,
    ) -> Step<ToolOutcome> {
        let mut auths = Vec::new();
        let command = match m.name.as_str() {
            "engine_transaction" => args["operations"].as_array().map(|ops| {
                let names: Vec<_> = ops
                    .iter()
                    .filter_map(|o| o["op"].as_str())
                    .take(8)
                    .collect();
                format!("{} operacji: {}", ops.len(), names.join(", "))
            }),
            "engine_shader" => args["shader_name"].as_str().map(|s| format!("shader {s}")),
            _ => None,
        };
        let auth = self.authorize(ctx, m, command).await?;
        let approval = auth.approval;
        auths.push(auth);
        let result = self.dispatch(m, args, ctx, &mut auths).await;
        self.release(auths).await;
        let mut out = result?;
        out.approval = out.approval.or(approval);
        if let Some(source) = m.untrusted_output.clone() {
            report_untrusted(&self.gate, ctx, source.clone()).await;
            out = out.untrusted(source);
        }
        Ok(out)
    }

    async fn dispatch(
        &self,
        m: &ToolManifest,
        args: Value,
        ctx: &ToolCtx,
        auths: &mut Vec<tools_common_contract::Authorization>,
    ) -> Step<ToolOutcome> {
        match m.name.as_str() {
            "engine_status" => Ok(self.status()),
            "engine_docs" => self.docs(parse_args(args)?).await,
            "engine_start" => {
                let a: StartArgs = parse_args(args)?;
                let (load, auth) = self.load_path(ctx, m, a.load.as_deref()).await?;
                auths.extend(auth);
                let options = StartOptions {
                    scene: a.scene.unwrap_or_default(),
                    load,
                    paused: a.paused.unwrap_or(true),
                    window: a.window.unwrap_or(false),
                    audio: a.audio.unwrap_or(false),
                };
                let state = self
                    .port
                    .start(options)
                    .await
                    .map_err(|e| engine_fail(&e))?;
                let text = format!(
                    "Silnik działa: {} (tryb {}, czas {}).",
                    state.url.as_deref().unwrap_or("?"),
                    state.mode.as_deref().unwrap_or("?"),
                    if state.paused {
                        "stoi do engine_step"
                    } else {
                        "płynie"
                    }
                );
                Ok(ToolOutcome::ok(text, json!(state)))
            }
            "engine_stop" => {
                let stopped = self.port.stop().await.map_err(|e| engine_fail(&e))?;
                let text = if stopped {
                    "Zatrzymano serwer silnika."
                } else {
                    "Serwer silnika nie działał."
                };
                Ok(ToolOutcome::ok(text, json!({"stopped": stopped})))
            }
            "engine_inspect" => self.inspect(parse_args(args)?).await,
            "engine_transaction" => {
                let a: TransactionArgs = parse_args(args)?;
                let id = a.transaction_id.clone().unwrap_or_else(|| {
                    derived_id(ctx, "tx", &json!([a.operations, a.expected_revision]))
                });
                let tx = json!({"transaction_id": id, "expected_revision": a.expected_revision,
                                "save_ram_snapshot": true, "operations": a.operations});
                let r = self.rpc(EngineMethod::ExecuteTransaction, tx).await?;
                let text = format!(
                    "Zastosowano {} operacji (transakcja {id}): rewizja {}, nowe encje {}. Cofnięcie: engine_rollback.",
                    a.operations.len(),
                    r["revision"],
                    r["spawned_entities"]
                );
                Ok(ToolOutcome::ok(text, r))
            }
            "engine_rollback" => {
                let r = self
                    .rpc(EngineMethod::RollbackLastChange, json!({}))
                    .await?;
                let text = format!(
                    "Cofnięto ostatnią zmianę: rewizja {}, w historii zostało {}.",
                    r["revision"], r["history_len"]
                );
                Ok(ToolOutcome::ok(text, r))
            }
            "engine_step" => {
                let a: StepArgs = parse_args(args)?;
                let r = self
                    .cancellable(ctx, EngineMethod::HeadlessStep, json!({"frames": a.frames}))
                    .await?;
                let text = format!(
                    "Symulacja: klatka {}, rewizja {}.",
                    r["frame"], r["revision"]
                );
                Ok(ToolOutcome::ok(text, r))
            }
            "engine_stream" => self.stream(parse_args(args)?, ctx).await,
            "engine_raycast" => {
                let a: RaycastArgs = parse_args(args)?;
                let params = json!({"dimension": a.dimension.unwrap_or_default(), "origin": a.origin,
                    "direction": a.direction, "max_distance": a.max_distance.unwrap_or(1000.0)});
                let r = self.rpc(EngineMethod::RaycastWorld, params).await?;
                let text = if r["hit"].is_null() {
                    "Promień niczego nie trafił.".to_owned()
                } else {
                    format!("Trafienie: {}", r["hit"])
                };
                Ok(ToolOutcome::ok(text, r))
            }
            "engine_height" => {
                let a: HeightArgs = parse_args(args)?;
                let r = self
                    .rpc(EngineMethod::QueryWorldHeight, json!({"x": a.x, "z": a.z}))
                    .await?;
                Ok(ToolOutcome::ok(
                    format!("Wysokość terenu: {} m.", r["height"]),
                    r,
                ))
            }
            "engine_select" => {
                let a: SelectArgs = parse_args(args)?;
                let r = self
                    .rpc(
                        EngineMethod::SetSpatialSelection,
                        json!({"entity_id": a.entity_id}),
                    )
                    .await?;
                let text = match a.entity_id {
                    Some(id) => format!("Zaznaczono encję {id}."),
                    None => "Wyczyszczono zaznaczenie.".into(),
                };
                Ok(ToolOutcome::ok(text, r))
            }
            "engine_shader" => {
                let a: ShaderArgs = parse_args(args)?;
                let id = a.transaction_id.clone().unwrap_or_else(|| {
                    derived_id(
                        ctx,
                        "shader",
                        &json!([a.shader_name, a.wgsl_code, a.expected_revision]),
                    )
                });
                let mut params = json!({"transaction_id": id, "shader_name": a.shader_name, "wgsl_code": a.wgsl_code});
                if let Some(rev) = a.expected_revision {
                    params["expected_revision"] = json!(rev);
                }
                let r = self.rpc(EngineMethod::HotReloadShader, params).await?;
                Ok(ToolOutcome::ok(
                    format!("Shader przyjęty: {}.", r["status"]),
                    r,
                ))
            }
            "engine_capture" => {
                let capture = self.port.capture().await.map_err(|e| engine_fail(&e))?;
                match capture {
                    Some(c) => capture_outcome(&c, "Zrzut okna silnika"),
                    None => Err(fail(
                        ToolErrorKind::NotFound,
                        "Brak zrzutu — uruchom silnik z `window: true` i wprowadź zmianę (zrzut powstaje przy każdej nowej rewizji).",
                    )),
                }
            }
            _ => {
                let a: PreviewArgs = parse_args(args)?;
                let (load, auth) = self.load_path(ctx, m, a.load.as_deref()).await?;
                auths.extend(auth);
                let options = PreviewOptions {
                    scene: if load.is_some() {
                        Scene::Empty
                    } else {
                        a.scene.unwrap_or(Scene::Demo)
                    },
                    load,
                    frames: a.frames.unwrap_or(3),
                    width: a.width.unwrap_or(1280),
                    height: a.height.unwrap_or(720),
                };
                let capture = tokio::select! {
                    c = self.port.preview(options) => c.map_err(|e| engine_fail(&e))?,
                    // Proces podglądu kończy się sam po `frames` klatkach (kill-switch zabija go od razu).
                    () = ctx.cancel.cancelled() => return Ok(ToolOutcome::cancelled(&m.title)),
                };
                capture_outcome(&capture, "Podgląd sceny")
            }
        }
    }

    pub(crate) async fn cancellable(
        &self,
        ctx: &ToolCtx,
        method: EngineMethod,
        params: Value,
    ) -> Step<Value> {
        tokio::select! {
            r = self.rpc(method, params) => r,
            () = ctx.cancel.cancelled() => Err(Box::new(ToolOutcome::cancelled(
                "czekanie na silnik (krok mógł się jeszcze wykonać)",
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use core_bus_contract::RunId;
    use safety_broker_contract::Holder;
    use serde_json::json;
    use tools_common_contract::ToolCtx;

    use super::derived_id;

    fn ctx(run: &str, step: u32) -> ToolCtx {
        let mut ctx = ToolCtx::new(Holder::agent("s1", "delta"));
        ctx.run = Some(RunId::new(run));
        ctx.step = step;
        ctx
    }

    /// Ponowienie po upływie terminu w następnym kroku trafia w ten sam identyfikator
    /// (przegląd 2026-10-27: numer kroku zmieniał id i zmiana stosowała się drugi raz).
    #[test]
    fn retry_in_next_step_keeps_the_transaction_id() {
        let body = json!([[{"op": "entity.spawn"}], null]);
        let id =
            |run, step, kind, body: &serde_json::Value| derived_id(&ctx(run, step), kind, body);
        assert_eq!(id("r1", 3, "tx", &body), id("r1", 4, "tx", &body));
        assert_ne!(id("r1", 3, "tx", &body), id("r2", 3, "tx", &body));
        let other = json!([[{"op": "entity.spawn"}], 7]);
        assert_ne!(id("r1", 3, "tx", &body), id("r1", 3, "tx", &other));
        assert_ne!(id("r1", 3, "tx", &body), id("r1", 3, "shader", &body));
    }
}
