//! Kontynuacja strumienia Dyrygentki z limitem czasu i publikacją tekstu.
use crate::engine::{ChatEngine, GenHandle};
use crate::generate::GenRequest;
use crate::stream::{merge_live, update_blocks, utf16_len};
use app_api::dto::AlfaEvent;
use app_api::ports::{BrainChoice, VoiceChunk};
use futures_util::StreamExt;
use lib_markdown::IncrementalRenderer;
use providers_contract::{AssistantTurn, ChatRequest, ProviderEvent, TurnAccumulator};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn followup(
    core: &ChatEngine,
    req: &GenRequest,
    handle: &GenHandle,
    choice: &BrainChoice,
    request: &ChatRequest,
    renderer: &mut IncrementalRenderer,
    sent_units: &mut u64,
    budget: &crate::conductor_budget::ConductorBudget,
) -> (AssistantTurn, bool, bool) {
    let sid = req.session.to_string();
    let tid = app_api::ids::turn_dto(&req.session, handle.turn);
    let mut cancelled = false;
    let mut exhausted = false;
    let mut stream = choice
        .provider
        .stream(request.clone(), handle.cancel.clone());
    let mut acc = TurnAccumulator::new(choice.provider.id().clone());
    let mut first_delta = true;
    let deadline = tokio::time::sleep(std::time::Duration::from_millis(
        budget.remaining().map_or(1, |b| b.max_wall_ms),
    ));
    tokio::pin!(deadline);
    loop {
        let ev = tokio::select! {
            biased;
            () = &mut deadline => {if let Some(next)=budget.remaining(){deadline.as_mut().reset(tokio::time::Instant::now()+std::time::Duration::from_millis(next.max_wall_ms.max(1)));continue;}exhausted = true; break; }
            () = handle.cancel.cancelled() => { cancelled = true; break; }
            next = stream.next() => match next { Some(e) => e, None => break },
        };
        acc.push(&ev);
        if let ProviderEvent::TextDelta { text, .. } = &ev {
            let piece = if first_delta && *sent_units > 0 && !renderer.source().ends_with('\n') {
                first_delta = false;
                format!("\n\n{text}")
            } else {
                first_delta = false;
                text.clone()
            };
            if let Some(tap) = &req.tap {
                let _ = tap.send(VoiceChunk::Text(piece.clone()));
            }
            let blocks = update_blocks(renderer.push(&piece));
            let offset = *sent_units;
            *sent_units += utf16_len(&piece);
            if let Ok(mut live) = handle.live.lock() {
                merge_live(&mut live, &piece, &blocks);
            }
            core.emit(AlfaEvent::TextDelta {
                session_id: sid.clone(),
                turn_id: tid.clone(),
                text: piece,
                blocks,
                offset,
            });
        }
    }
    drop(stream);
    (acc.finish(), cancelled, exhausted)
}
