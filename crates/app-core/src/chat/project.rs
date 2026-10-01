//! Projekcja tur `sessions` (+ fakty `app-core`) do DTO `Turn` i bloków HTML (lib-markdown).

use lib_markdown::{Block, RenderOptions, render_blocks};
use sessions_contract::{Author, Role, SessionId, Turn};

use crate::dto::{self, BlockKind, RenderedBlock, ThinkingInfo, TurnStatus};
use crate::ids;
use crate::store::TurnMeta;

/// Blok lib-markdown → DTO (blok kodu = jeden fenced block najwyższego poziomu).
pub fn block_dto(block: &Block, closed: bool) -> RenderedBlock {
    let is_code = block.code.len() == 1 && block.html.trim_start().starts_with("<pre");
    RenderedBlock {
        index: block.id,
        kind: if is_code {
            BlockKind::Code
        } else {
            BlockKind::Text
        },
        lang: if is_code {
            block.code.first().and_then(|c| c.lang.clone())
        } else {
            None
        },
        html_sanitized: block.html.clone(),
        closed,
    }
}

/// Cały tekst → zamknięte bloki.
pub fn render_closed(text: &str) -> Vec<RenderedBlock> {
    render_blocks(text, RenderOptions::default())
        .iter()
        .map(|b| block_dto(b, true))
        .collect()
}

/// Autor tury w DTO.
pub fn author_of(turn: &Turn, meta: Option<&TurnMeta>) -> String {
    match &turn.author {
        Author::User => "user".to_owned(),
        Author::Agent { agent } => agent.to_string(),
        _ => meta
            .and_then(|m| m.agent.clone())
            .unwrap_or_else(|| "alfa".to_owned()),
    }
}

/// Tura z bazy → DTO. `logged` = ostatni stan z logu (kolejka offline).
pub fn turn_dto(
    session: &SessionId,
    turn: &Turn,
    meta: Option<&TurnMeta>,
    logged: Option<TurnStatus>,
) -> dto::Turn {
    // Tura-komunikat systemowy = odpowiedź, która nie powstała (błąd, anulowanie przed tekstem).
    let marker = turn.role == Role::System;
    let text = if marker {
        String::new()
    } else {
        turn.content.text.clone()
    };
    let status = logged
        .or_else(|| meta.and_then(|m| m.status))
        .unwrap_or(TurnStatus::Complete);
    let usage = meta.and_then(|m| m.usage.clone());
    dto::Turn {
        id: ids::turn_dto(session, turn.id),
        session_id: session.to_string(),
        parent_id: turn.parent.map(|p| ids::turn_dto(session, p)),
        author: author_of(turn, meta),
        role_id: meta.and_then(|m| m.role_id.clone()),
        created_at: dto::iso(turn.created_at),
        status,
        blocks: render_closed(&text),
        text,
        thinking: meta.and_then(|m| m.thinking_ms).map(|ms| ThinkingInfo {
            duration_ms: ms,
            active: false,
        }),
        tools: Vec::new(),
        approval: None,
        usage,
        error: meta.and_then(|m| m.error.clone()),
        continues: meta
            .and_then(|m| m.continues)
            .map(|c| ids::turn_dto(session, sessions_contract::TurnId(c))),
        addressed_to: meta.and_then(|m| m.addressed_to.clone()),
        truncated: meta.is_some_and(|m| m.truncated),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_blocks_are_marked_with_language() {
        let blocks = render_closed("Tekst **gruby**\n\n```sql\nSELECT 1;\n```\n");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Text);
        assert_eq!(blocks[1].kind, BlockKind::Code);
        assert_eq!(blocks[1].lang.as_deref(), Some("sql"));
        assert!(blocks.iter().all(|b| b.closed));
        assert!(!blocks[0].html_sanitized.contains("<script"));
    }

    #[test]
    fn hostile_markdown_is_sanitized() {
        let blocks = render_closed("<script>alert(1)</script>\n\n[x](javascript:alert(1))");
        let html: String = blocks.iter().map(|b| b.html_sanitized.as_str()).collect();
        assert!(!html.contains("<script"));
        assert!(!html.contains("javascript:"));
    }
}
