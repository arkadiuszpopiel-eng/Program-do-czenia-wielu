//! Końcowy tekst orkiestracji: strumień albo wyniki ukończonych podzadań.
use crate::engine::{ChatEngine, GenHandle};
use crate::project::render_closed;
use crate::stream::update_blocks;
use app_api::dto::AlfaEvent;
use lib_markdown::IncrementalRenderer;
use providers_contract::AssistantTurn;

/// Czyści tekst wstępny, gdy uruchamiane jest awaryjne delegowanie.
pub(crate) fn reset(handle: &GenHandle, renderer: &mut IncrementalRenderer, sent_units: &mut u64) {
    *renderer = IncrementalRenderer::new(lib_markdown::RenderOptions::default());
    *sent_units = 0;
    if let Ok(mut live) = handle.live.lock() {
        live.text.clear();
        live.blocks.clear();
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finish(
    core: &ChatEngine,
    handle: &GenHandle,
    sid: String,
    tid: String,
    renderer: IncrementalRenderer,
    sent_units: u64,
    current_turn: &AssistantTurn,
    summaries: &[String],
    evidence: &[crate::conductor_evidence::Receipt],
) -> String {
    let mut text = renderer.source().to_owned();
    let closing = update_blocks(renderer.finish());
    if !closing.is_empty() {
        core.emit(AlfaEvent::TextDelta {
            session_id: sid.clone(),
            turn_id: tid.clone(),
            text: String::new(),
            blocks: closing,
            offset: sent_units,
        });
    }
    if text.trim().is_empty() {
        text = current_turn.message.visible_text();
    }
    if text.trim().is_empty() && !summaries.is_empty() {
        text = summaries.join("\n\n");
        let blocks = render_closed(&text);
        if let Ok(mut live) = handle.live.lock() {
            live.text.clone_from(&text);
            live.blocks.clone_from(&blocks);
        }
        core.emit(AlfaEvent::TextDelta {
            session_id: sid.clone(),
            turn_id: tid.clone(),
            text: text.clone(),
            blocks,
            offset: 0,
        });
    }
    let footer = crate::conductor_evidence::footer(evidence);
    if !footer.is_empty() {
        let offset = crate::stream::utf16_len(&text);
        text.push_str(&footer);
        let blocks = render_closed(&text);
        if let Ok(mut live) = handle.live.lock() {
            live.text = text.clone();
            live.blocks = blocks.clone();
        }
        core.emit(AlfaEvent::TextDelta {
            session_id: sid,
            turn_id: tid,
            text: footer,
            blocks,
            offset,
        });
    }
    text
}
