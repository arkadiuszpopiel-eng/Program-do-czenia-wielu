//! Pipeline modeli (`run_pipeline`): łańcuch etapów, w którym każdy etap wykonuje wskazana rola
//! agentki na wskazanym modelu (`dostawca:model`), a wynik etapu trafia jako wejście następnego —
//! bez pośrednictwa właściciela. Przykład: szkic → model od stylu i typografii → model od
//! architektury komponentów i dostępności → model piszący końcowy kod Tailwind/React.
//! Etapy to zwykłe podzadania Dyrygentki (kroki w turze i w Replay, koszt per model).
//! Pipeline'y wybiera się po nazwie z biblioteki (`crate::pipelines`: własne + wbudowane).

use std::collections::BTreeMap;
use std::time::Instant;

use app_agents::short;
use app_api::dto::{AlfaEvent, PipelineDef, ToolIcon, ToolStatus, ToolStep};
use app_api::ids;
use providers_contract::ToolSpec;
use serde_json::Value;

use crate::conductor_child::{
    ChildExec, execute_child, record_delegation_step, resolve_delegation,
};
use crate::outcome::millis;
use crate::pipelines::{self, MAX_STAGES, clean_model};

/// Nazwa narzędzia pipeline'u.
pub(crate) const PIPELINE_TOOL: &str = "run_pipeline";

/// Jeden etap pipeline'u po rozwiązaniu pipeline'u z biblioteki / listy `steps`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Stage {
    pub title: String,
    pub role: String,
    pub persona: Option<String>,
    pub model: Option<String>,
    pub goal: String,
}

fn text_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
}

fn stages_of(def: &PipelineDef) -> Vec<Stage> {
    def.stages
        .iter()
        .map(|s| Stage {
            title: s.title.clone(),
            role: s.role.clone(),
            persona: s.persona.clone(),
            model: s.model.clone(),
            goal: s.goal.clone(),
        })
        .collect()
}

/// Buduje listę etapów z argumentów narzędzia: `preset` (nazwa z biblioteki) albo `steps`;
/// `models` nadpisuje modele kolejnych etapów (pozycyjnie).
pub(crate) fn plan(input: &Value, library: &[PipelineDef]) -> Result<Vec<Stage>, String> {
    let mut stages = match input.get("steps").and_then(Value::as_array) {
        Some(steps) if !steps.is_empty() => {
            let mut out = Vec::new();
            for (i, s) in steps.iter().enumerate() {
                let goal = ["goal", "task", "zadanie"]
                    .iter()
                    .map(|k| text_of(s, k))
                    .find(|g| !g.is_empty())
                    .ok_or_else(|| format!("Etap {} nie ma celu (`goal`).", i + 1))?;
                let role = text_of(s, "role");
                let persona = text_of(s, "persona");
                if role.is_empty() && persona.is_empty() {
                    return Err(format!("Etap {} wymaga `role` lub `persona`.", i + 1));
                }
                out.push(Stage {
                    title: text_of(s, "title").to_owned(),
                    role: if role.is_empty() {
                        persona.to_owned()
                    } else {
                        role.to_owned()
                    },
                    persona: (!persona.is_empty()).then(|| persona.to_owned()),
                    model: clean_model(text_of(s, "model"))
                        .map_err(|e| format!("Etap {}: {e}", i + 1))?,
                    goal: goal.to_owned(),
                });
            }
            out
        }
        _ => {
            let name = ["pipeline", "preset"]
                .iter()
                .map(|k| text_of(input, k))
                .find(|n| !n.is_empty())
                .unwrap_or_default();
            let found = pipelines::find(library, name).ok_or_else(|| {
                let names: Vec<&str> = library.iter().map(|d| d.name.as_str()).collect();
                format!(
                    "Podaj `steps` albo `pipeline` (dostępne: {}).",
                    names.join(", ")
                )
            })?;
            stages_of(found)
        }
    };
    if stages.len() > MAX_STAGES {
        return Err(format!(
            "Za dużo etapów ({}); maksymalnie {MAX_STAGES}.",
            stages.len()
        ));
    }
    if let Some(models) = input.get("models").and_then(Value::as_array) {
        for (st, m) in stages.iter_mut().zip(models) {
            if let Some(raw) = m.as_str()
                && let Some(model) = clean_model(raw)?
            {
                st.model = Some(model);
            }
        }
    }
    Ok(stages)
}

/// Wstawia wejście użytkownika (`{{input}}`) i wynik poprzedniego etapu (`{{prev}}`); gdy szablon
/// nie ma znaczników, dopisuje wejście na końcu. Wynik poprzedniego etapu trafia do celu zawsze;
/// `{{prev}}` wpisane w treść wejścia zostaje tekstem (podstawienie jednoprzebiegowe).
pub(crate) fn render(template: &str, input: &str, prev: Option<&str>) -> String {
    let has_prev = template.contains("{{prev}}");
    let body = template
        .split("{{prev}}")
        .map(|part| part.replace("{{input}}", input))
        .collect::<Vec<_>>()
        .join(prev.unwrap_or(input));
    match prev {
        Some(p) if !has_prev => format!("{body}\n\nWynik poprzedniego etapu:\n{p}"),
        None if !has_prev && !template.contains("{{input}}") => {
            format!("{body}\n\nWejście:\n{input}")
        }
        _ => body,
    }
}

/// Wynik etapu bez nagłówka `[Imię (Rola)]: ` dodawanego przez delegację.
fn body_of(result: &str) -> &str {
    match result.strip_prefix('[').and_then(|r| r.split_once("]: ")) {
        Some((_, rest)) => rest,
        None => result,
    }
}

/// Opis narzędzia `run_pipeline` dla Dyrygentki (z listą pipeline'ów z biblioteki).
pub(crate) fn pipeline_tool_spec(library: &[PipelineDef]) -> ToolSpec {
    let list: Vec<String> = library
        .iter()
        .map(|d| {
            let chain: Vec<&str> = d.stages.iter().map(|s| s.role.as_str()).collect();
            format!("`{}` ({})", d.name, chain.join(" → "))
        })
        .collect();
    ToolSpec {
        name: PIPELINE_TOOL.to_owned(),
        description: format!(
            "Uruchamia pipeline: kolejne etapy wykonują wskazane role na wskazanych modelach, a \
             wynik każdego etapu automatycznie trafia do następnego (bez udziału właściciela). \
             Używaj, gdy właściciel opisuje łańcuch modeli/etapów („A → B → C”), prosi o pipeline \
             albo wskazuje pipeline po nazwie. Podaj `input` (szkic/opis/zadanie) oraz `pipeline` \
             (nazwa z biblioteki: {}) albo własne `steps`. Model etapu w zapisie \
             `dostawca:model`; `models` nadpisuje modele etapów pipeline'u po kolei. W celu etapu \
             możesz użyć {{{{input}}}} i {{{{prev}}}}.",
            list.join(", ")
        ),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "input": { "type": "string", "description": "Szkic, opis lub zadanie wejściowe pipeline'u." },
                "pipeline": { "type": "string", "description": "Nazwa pipeline'u z biblioteki (własnego lub wbudowanego)." },
                "models": { "type": "array", "items": { "type": "string" }, "description": "Modele kolejnych etapów pipeline'u z biblioteki (dostawca:model)." },
                "steps": {
                    "type": "array",
                    "description": "Własne etapy (maks. 6), wykonywane po kolei.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "description": "Krótka nazwa etapu." },
                            "role": { "type": "string", "description": "Rola: coder, operator, researcher, critic, thinker, writer, keeper." },
                            "persona": { "type": "string", "description": "Opcjonalnie agentka (delta, gama, beta)." },
                            "model": { "type": "string", "description": "Opcjonalnie dostawca:model; pusty = dobór Routera." },
                            "goal": { "type": "string", "description": "Zadanie etapu; może zawierać {{input}} i {{prev}}." }
                        },
                        "required": ["goal"]
                    }
                }
            },
            "required": ["input"]
        }),
        strict: false,
    }
}

/// Zdanie do promptu Dyrygentki o pipeline'ach.
pub(crate) fn pipeline_note() -> &'static str {
    " Gdy właściciel opisuje łańcuch modeli lub etapów (np. „szkic → model A → model B → model C”), \
     prosi o pipeline albo wskazuje pipeline po nazwie, wywołaj `run_pipeline` (`pipeline` z \
     biblioteki albo `steps` z polem `model` w zapisie `dostawca:model`) — etapy przekazują sobie \
     wyniki same, nie pytaj o pośrednie kroki."
}

/// Liczniki zużycia pipeline'u: tokeny wejściowe, wyjściowe, koszt (nano USD).
pub(crate) type Usage3 = (u64, u64, u64);

/// Wykonuje pipeline etap po etapie; zwraca tekst wyniku narzędzia i znacznik błędu.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_pipeline_call(
    cx: &ChildExec<'_>,
    parent_id: &str,
    parent_raw: &str,
    started: Instant,
    input: &Value,
    step_n: &mut u32,
    all_tools: &mut Vec<ToolStep>,
    by_model: &mut BTreeMap<String, (u64, u64, Option<u64>)>,
    usage: &mut Usage3,
) -> (String, bool) {
    let user_input = text_of(input, "input").to_owned();
    if user_input.is_empty() {
        return ("Pusty `input` pipeline'u.".into(), true);
    }
    let library = cx.core.pipeline_library().await;
    let stages = match input.get("__stages").filter(|v| !v.is_null()).map_or_else(
        || plan(input, &library),
        |v| serde_json::from_value(v.clone()).map_err(|e| format!("Nieczytelny zapis etapów: {e}")),
    ) {
        Ok(s) => s,
        Err(e) => return (e, true),
    };
    let sid = cx.req.session.to_string();
    let total = stages.len();
    let start = input["__stage"].as_u64().unwrap_or(0) as usize;
    let mut prev: Option<String> = input["__prev"].as_str().map(str::to_owned);
    let mut log: Vec<String> = serde_json::from_value(input["__log"].clone()).unwrap_or_default();
    for (i, st) in stages.iter().enumerate().skip(start) {
        if cx.handle.cancel.is_cancelled() {
            return ("Pipeline przerwany przez użytkownika.".into(), true);
        }
        let already_started = i == start && input["__stage_started"].as_bool().unwrap_or(false);
        if !already_started && !cx.budget.step() {
            cx.budget.pause();
            return (
                format!(
                    "Pipeline zatrzymany przed etapem {}/{total}: wspólny limit zadania został osiągnięty.",
                    i + 1
                ),
                true,
            );
        }
        if !already_started {
            *step_n += 1;
        }
        if let Err(error) = crate::child_state::pipeline_cursor(
            cx,
            &stages,
            i,
            prev.as_deref(),
            &log,
            true,
            (*step_n, all_tools),
        ) {
            cx.budget.fail_storage(&error);
            return (format!("Nie zapisano etapu: {error}"), true);
        }
        let step_id = ids::step_dto(&cx.req.session, parent_raw, *step_n);
        let at_ms = millis(started.elapsed());
        let args = serde_json::json!({
            "role": st.role,
            "persona": st.persona,
            "goal": render(&st.goal, &user_input, prev.as_deref()),
        });
        let (persona, roles, goal) =
            match resolve_delegation(cx.core, &cx.req.session, &cx.req.agent, &args) {
                Ok(x) => x,
                Err(e) => {
                    return (
                        format!("Pipeline przerwany na etapie {}/{total}: {e}", i + 1),
                        true,
                    );
                }
            };
        let role_label = roles
            .first()
            .map_or_else(|| persona.name.clone(), |r| r.name.clone());
        let model_note = st
            .model
            .as_deref()
            .map(|m| format!(" · {m}"))
            .unwrap_or_default();
        let name = if st.title.is_empty() {
            short(&goal, 60)
        } else {
            st.title.clone()
        };
        let title = format!(
            "Pipeline {}/{total} → {} ({role_label}{model_note}): {name}",
            i + 1,
            persona.name
        );
        let line = ToolStep {
            id: step_id.clone(),
            icon: ToolIcon::Edit,
            label: title.clone(),
            status: ToolStatus::Running,
            duration_ms: None,
            undo_token: None,
            undone: false,
            intent: None,
        };
        all_tools.push(line.clone());
        sync_live(cx, all_tools);
        cx.core.emit(AlfaEvent::ToolCall {
            session_id: sid.clone(),
            turn_id: cx.tid.to_owned(),
            step: line,
        });
        let mut stage_cx = cx.clone();
        stage_cx.call_id = format!("{}-stage-{i}", cx.call_id);
        if i != start {
            stage_cx.input = serde_json::json!({});
        }
        let (res, cu) = execute_child(
            &stage_cx,
            persona,
            roles,
            goal.clone(),
            all_tools,
            by_model,
            st.model.clone(),
        )
        .await;
        usage.0 += cu.input_tokens;
        usage.1 += cu.output_tokens;
        usage.2 = usage.2.saturating_add(cu.cost_nano_usd);
        let dur = millis(started.elapsed()).saturating_sub(at_ms);
        let (txt, err) = match &res {
            Ok(t) => (t.clone(), false),
            Err(e) => (e.clone(), true),
        };
        record_delegation_step(
            cx.core,
            &cx.req.session,
            parent_id,
            cx.tid,
            step_id,
            *step_n,
            title,
            &goal,
            &txt,
            err,
            at_ms,
            dur,
            all_tools,
        );
        sync_live(cx, all_tools);
        if err {
            return (
                format!("Pipeline przerwany na etapie {}/{total}: {txt}", i + 1),
                true,
            );
        }
        let out = body_of(&txt).to_owned();
        log.push(format!("Etap {}/{total}: {}", i + 1, short(&out, 600)));
        prev = Some(out);
        if let Err(error) = crate::child_state::pipeline_cursor(
            cx,
            &stages,
            i + 1,
            prev.as_deref(),
            &log,
            false,
            (*step_n, all_tools),
        ) {
            cx.budget.fail_storage(&error);
            return (format!("Nie zapisano wyniku etapu: {error}"), true);
        }
    }
    let last = prev.unwrap_or_default();
    let intermediate = log[..log.len().saturating_sub(1)].join("\n");
    let text = format!(
        "Pipeline zakończony ({total} etapów). Wynik końcowy:\n{last}\n\nPrzebieg:\n{intermediate}"
    );
    (text, false)
}

fn sync_live(cx: &ChildExec<'_>, all_tools: &[ToolStep]) {
    if let Ok(mut live) = cx.handle.live.lock() {
        live.tools = all_tools.to_vec();
    }
}

pub(crate) use crate::conductor_spec::{is_conductor_tool, model_of};

#[cfg(test)]
#[path = "conductor_pipeline_tests.rs"]
mod tests;
