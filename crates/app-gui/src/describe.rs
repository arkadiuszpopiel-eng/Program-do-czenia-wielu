//! Opis akcji GUI dla panelu „Ekran" — rodzaj, cel, skrót **bez wpisywanej treści** (np. „Wpisz
//! tekst (N znaków)"), metadane zrzutu i wynik przy przejęciu sterowania (wydzielone z `monitor.rs`).

use std::time::Duration;

use app_api::dto::{GuiAction, GuiActionStatus, GuiShotInfo, iso};
use chrono::Utc;
use tools_common_contract::{DenialReason, ToolCtx, ToolManifest, ToolOutcome, ToolStatus};

use crate::monitor::SCREEN_TOOL;

pub(crate) fn agent_of(ctx: &ToolCtx) -> String {
    ctx.holder
        .agent
        .as_ref()
        .map_or_else(|| "?".to_owned(), ToString::to_string)
}

pub(crate) fn action(
    id: u64,
    ctx: &ToolCtx,
    m: &ToolManifest,
    status: GuiActionStatus,
    took: Option<Duration>,
) -> GuiAction {
    GuiAction {
        id,
        at: iso(Utc::now()),
        session_id: ctx.holder.session.to_string(),
        agent: agent_of(ctx),
        tool: m.name.clone(),
        title: m.title.clone(),
        target: None,
        status,
        summary: m.title.clone(),
        duration_ms: took.map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
    }
}

pub(crate) fn status_of(s: &ToolStatus) -> GuiActionStatus {
    match s {
        ToolStatus::Ok => GuiActionStatus::Ok,
        ToolStatus::Denied { .. } | ToolStatus::NeedsConfirmation => GuiActionStatus::Denied,
        ToolStatus::Cancelled => GuiActionStatus::Cancelled,
        _ => GuiActionStatus::Failed,
    }
}

/// Aplikacja celu z danych wyniku (`app` na dowolnym poziomie do głębokości 3) — bez tytułów okien.
pub(crate) fn target_of(data: &serde_json::Value) -> Option<String> {
    fn find(v: &serde_json::Value, depth: u32) -> Option<String> {
        let obj = v.as_object()?;
        if let Some(app) = obj.get("app").and_then(serde_json::Value::as_str) {
            return Some(app.to_owned()).filter(|a| !a.is_empty());
        }
        if depth == 0 {
            return None;
        }
        obj.values().find_map(|c| find(c, depth - 1))
    }
    find(data, 3)
}

/// Opis akcji bez treści: wpisywany tekst tylko jako liczba znaków, klik — współrzędne.
pub(crate) fn summary(m: &ToolManifest, args: &serde_json::Value, out: &ToolOutcome) -> String {
    let base = match m.name.as_str() {
        "input_type_text" => {
            let n = args["text"].as_str().map_or(0, |t| t.chars().count());
            format!("{} ({n} znaków)", m.title)
        }
        "input_click" => match (args["x"].as_i64(), args["y"].as_i64()) {
            (Some(x), Some(y)) => format!("{} ({x}, {y})", m.title),
            _ => m.title.clone(),
        },
        SCREEN_TOOL => match (out.data["width"].as_u64(), out.data["height"].as_u64()) {
            (Some(w), Some(h)) => format!(
                "{} {w}×{h}, zamaskowano {}",
                m.title,
                out.data["masked"].as_array().map_or(0, Vec::len)
            ),
            _ => m.title.clone(),
        },
        _ => m.title.clone(),
    };
    match &out.status {
        ToolStatus::Denied { .. } => format!("{base} — odmowa"),
        ToolStatus::Cancelled => format!("{base} — anulowano"),
        ToolStatus::Failed { .. } => format!("{base} — błąd"),
        _ => base,
    }
}

pub(crate) fn shot_info(ctx: &ToolCtx, data: &serde_json::Value) -> GuiShotInfo {
    let num = |k: &str| u32::try_from(data[k].as_u64().unwrap_or(0)).unwrap_or(u32::MAX);
    GuiShotInfo {
        at: iso(Utc::now()),
        session_id: ctx.holder.session.to_string(),
        agent: agent_of(ctx),
        width: num("width"),
        height: num("height"),
        masked: u32::try_from(data["masked"].as_array().map_or(0, Vec::len)).unwrap_or(u32::MAX),
        black_frame: data["black_frame"].as_bool().unwrap_or(false),
    }
}

/// Wynik dla modelu, gdy właściciel przejął sterowanie.
pub(crate) fn taken_over_outcome(action: &str) -> ToolOutcome {
    let mut out = ToolOutcome::denied(DenialReason::Policy, action);
    out.text = "Właściciel przejął sterowanie ekranem — akcje GUI są wstrzymane, dopóki nie odda \
                sterowania w panelu „Ekran”. Nie ponawiaj tej akcji; zakończ zadanie albo napisz, \
                co zostało do zrobienia."
        .into();
    out
}
