//! Trwała orkiestracja Dyrygentki: delegacje, pipeline i projekcja postępu do rozmowy.
use crate::conductor_child::{
    ChildExec, execute_child, record_delegation_step, resolve_delegation,
};
pub(crate) use crate::conductor_pipeline::pipeline_tool_spec;
use crate::conductor_pipeline::{PIPELINE_TOOL, model_of, run_pipeline_call};
pub(crate) use crate::conductor_spec::{
    crew_note, delegate_tool_spec, ensure_delegate_call_if_handoff, has_delegate_calls,
};
use crate::engine::{ChatEngine, GenHandle};
use crate::generate::GenRequest;
use crate::outcome::{Chosen, Outcome, millis};
use app_agents::short;
use app_api::dto::{AlfaEvent, ToolIcon, ToolStatus, ToolStep};
use app_api::ids;
use app_api::ports::BrainChoice;
use lib_markdown::IncrementalRenderer;
use providers_contract::{
    AssistantTurn, ChatRequest, ContentBlock, Message, Role as MsgRole, ToolResult, ToolResultPart,
};
use std::time::Instant;
/// Wykonuje wieloetapową orkiestrację Dyrygentki po wywołaniu `delegate_task`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_delegated(
    core: &ChatEngine,
    req: &GenRequest,
    handle: &GenHandle,
    choice: BrainChoice,
    request: ChatRequest,
    current_turn: AssistantTurn,
    chosen: Chosen,
    started: Instant,
    mut renderer: IncrementalRenderer,
    mut sent_units: u64,
    clear_initial_text: bool,
) -> Outcome {
    if clear_initial_text {
        crate::conductor_summary::reset(handle, &mut renderer, &mut sent_units);
    }
    let sid = req.session.to_string();
    let tid = handle.live.lock().map(|l| l.id.clone()).unwrap_or_default();
    let workdir = core.session_workdir(&req.session).unwrap_or_default();
    let crate::conductor_init::Ready {
        mut work,
        budget,
        billed_before,
        parts_before,
        base_elapsed,
    } = crate::conductor_init::prepare(crate::conductor_init::Init {
        core,
        req,
        handle,
        choice: &choice,
        request,
        current_turn,
        model: &chosen.model,
        started,
    })
    .await;
    let mut cancelled = false;
    let mut exhausted = false;
    crate::conductor_state::persist(core, req, &tid, &mut work, &budget, base_elapsed);
    loop {
        work.append_current_message();
        if work.pending.is_empty() && work.results.is_empty() && !work.awaiting_model {
            break;
        }
        while let Some(mut u) = work.pending.first().cloned() {
            if handle.cancel.is_cancelled() {
                cancelled = true;
                break;
            }
            if !u.input["__started"].as_bool().unwrap_or(false) && !budget.step() {
                exhausted = true;
                break;
            }
            if !u.input["__started"].as_bool().unwrap_or(false) {
                u.input["__started"] = true.into();
                u.input["__sequence"] = (work.step + 1).into();
                work.pending[0] = u.clone();
            }
            if !crate::conductor_state::persist(core, req, &tid, &mut work, &budget, base_elapsed) {
                exhausted = true;
                break;
            }
            if u.name == PIPELINE_TOOL {
                let cx = ChildExec {
                    input: u.input.clone(),
                    call_id: format!(
                        "{}-{}",
                        u.id,
                        u.input["__sequence"]
                            .as_u64()
                            .unwrap_or(u64::from(work.step))
                    ),
                    budget: &budget,
                    core,
                    req,
                    handle,
                    parent_run_id: &work.parent.id,
                    tid: &tid,
                    workdir: &workdir,
                    history: work.base_history.clone(),
                };
                let mut usage = (0u64, 0u64, 0u64);
                let (txt, err) = run_pipeline_call(
                    &cx,
                    &work.parent.id,
                    &work.parent_raw,
                    started,
                    &u.input,
                    &mut work.step,
                    &mut work.tools,
                    &mut work.by_model,
                    &mut usage,
                )
                .await;
                work.usage.input_tokens += usage.0;
                work.usage.output_tokens += usage.1;
                work.usage.cost_nano_usd = work.usage.cost_nano_usd.saturating_add(usage.2);
                if !err {
                    work.summaries.push(txt.clone());
                }
                if budget.is_paused() {
                    exhausted = true;
                    crate::conductor_state::persist(
                        core,
                        req,
                        &tid,
                        &mut work,
                        &budget,
                        base_elapsed,
                    );
                    break;
                }
                work.results.push(ContentBlock::ToolResult(ToolResult {
                    tool_use_id: u.id,
                    content: vec![ToolResultPart::Text { text: txt }],
                    is_error: err,
                }));
                work.pending.remove(0);
                crate::conductor_state::persist(core, req, &tid, &mut work, &budget, base_elapsed);
                continue;
            }
            work.step = work.step.max(
                u.input["__sequence"]
                    .as_u64()
                    .unwrap_or(u64::from(work.step + 1)) as u32,
            );
            let step_id = ids::step_dto(&req.session, &work.parent_raw, work.step);
            let at_ms = millis(started.elapsed());
            let (out_text, is_err) =
                match resolve_delegation(core, &req.session, &req.agent, &u.input) {
                    Ok((persona, roles, goal)) => {
                        let role_label = roles
                            .first()
                            .map_or_else(|| persona.name.clone(), |r| r.name.clone());
                        let title = format!(
                            "Zlecenie → {} ({}): {}",
                            persona.name,
                            role_label,
                            short(&goal, 70)
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
                        work.tools.push(line.clone());
                        if let Ok(mut live) = handle.live.lock() {
                            live.tools.clone_from(&work.tools);
                        }
                        core.emit(AlfaEvent::ToolCall {
                            session_id: sid.clone(),
                            turn_id: tid.clone(),
                            step: line,
                        });
                        let cx = ChildExec {
                            input: u.input.clone(),
                            call_id: format!(
                                "{}-{}",
                                u.id,
                                u.input["__sequence"]
                                    .as_u64()
                                    .unwrap_or(u64::from(work.step))
                            ),
                            budget: &budget,
                            core,
                            req,
                            handle,
                            parent_run_id: &work.parent.id,
                            tid: &tid,
                            workdir: &workdir,
                            history: work.base_history.clone(),
                        };
                        let (res, cu) = execute_child(
                            &cx,
                            persona,
                            roles,
                            goal.clone(),
                            &mut work.tools,
                            &mut work.by_model,
                            model_of(&u.input),
                        )
                        .await;
                        work.usage.input_tokens += cu.input_tokens;
                        work.usage.output_tokens += cu.output_tokens;
                        work.usage.cost_nano_usd =
                            work.usage.cost_nano_usd.saturating_add(cu.cost_nano_usd);
                        let dur = millis(started.elapsed()).saturating_sub(at_ms);
                        let (txt, err) = match res {
                            Ok(t) => {
                                work.summaries.push(t.clone());
                                (t, false)
                            }
                            Err(e) => (e, true),
                        };
                        record_delegation_step(
                            core,
                            &req.session,
                            &work.parent.id,
                            &tid,
                            step_id,
                            work.step,
                            title,
                            &goal,
                            &txt,
                            err,
                            at_ms,
                            dur,
                            &mut work.tools,
                        );
                        if let Ok(mut live) = handle.live.lock() {
                            live.tools.clone_from(&work.tools);
                        }
                        (txt, err)
                    }
                    Err(e) => (e, true),
                };
            if budget.is_paused() {
                exhausted = true;
                crate::conductor_state::persist(core, req, &tid, &mut work, &budget, base_elapsed);
                break;
            }
            work.results.push(ContentBlock::ToolResult(ToolResult {
                tool_use_id: u.id,
                content: vec![ToolResultPart::Text { text: out_text }],
                is_error: is_err,
            }));
            work.pending.remove(0);
            crate::conductor_state::persist(core, req, &tid, &mut work, &budget, base_elapsed);
        }
        if cancelled || exhausted {
            break;
        }
        let Some(_remaining) = budget.remaining() else {
            exhausted = true;
            break;
        };
        if !work.awaiting_model {
            work.request.messages.push(Message::new(
                MsgRole::User,
                std::mem::take(&mut work.results),
            ));
            work.awaiting_model = true;
        }
        if !crate::conductor_state::persist(core, req, &tid, &mut work, &budget, base_elapsed) {
            exhausted = true;
            break;
        }
        (work.current, cancelled, exhausted) = crate::conductor_stream::followup(
            core,
            req,
            handle,
            &choice,
            &crate::conductor_handoff::parent_request(&work.request, &work.evidence),
            &mut renderer,
            &mut sent_units,
            &budget,
        )
        .await;
        let model_cost = work
            .current
            .model
            .as_deref()
            .and_then(|m| choice.provider.cost(m, &work.current.usage))
            .or_else(|| choice.provider.cost(&chosen.model, &work.current.usage))
            .map_or(0, |c| c.nano_usd);
        budget.charge_turn(&work.current, model_cost);
        work.awaiting_model = false;
        work.message_added = false;
        work.pending = crate::conductor_state::calls(&work.current);
        if exhausted {
            let partial = work.current.message.visible_text();
            if !partial.is_empty() {
                work.request.messages.push(Message::assistant_text(partial));
            }
            work.request.messages.push(Message::user_text("Kontynuuj zapisane zadanie od ostatniego punktu po zgodzie właściciela na przedłużenie czasu. Nie ponawiaj ukończonych operacji."));
            work.awaiting_model = true;
            work.message_added = true;
            work.pending.clear();
        }
        work.usage.input_tokens += work.current.usage.total_input();
        work.usage.output_tokens += work.current.usage.output_tokens;
        work.usage.cost_nano_usd = work.usage.cost_nano_usd.saturating_add(model_cost);
        crate::conductor_state::persist(core, req, &tid, &mut work, &budget, base_elapsed);
        if cancelled || exhausted || !has_delegate_calls(&work.current.message) {
            break;
        }
    }

    work.complete = cancelled || (!exhausted && work.current.error.is_none());
    crate::conductor_state::persist(core, req, &tid, &mut work, &budget, base_elapsed);
    crate::conductor_finish::finish(crate::conductor_finish::Completion {
        core,
        req,
        handle,
        work,
        budget: &budget,
        renderer,
        sent_units,
        started,
        chosen,
        routed: choice.routed,
        cancelled,
        exhausted,
        billed_before,
        parts_before,
    })
}
