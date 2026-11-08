use super::*;
use serde_json::json;

fn parse(lines: &[Value]) -> (ClaudeParser, Vec<AgentEvent>) {
    let mut p = ClaudeParser::new(Path::new("/wt/1"));
    let mut events = Vec::new();
    for l in lines {
        events.extend(p.line(&l.to_string()).events);
    }
    (p, events)
}

#[test]
fn full_stream_maps_to_events() {
    let (p, ev) = parse(&[
        json!({"type": "system", "subtype": "init", "session_id": "s-1",
               "mcp_servers": [{"name": "alfa", "status": "connected"}]}),
        json!({"type": "assistant", "message": {"content": [
            {"type": "thinking", "thinking": "myślę"},
            {"type": "text", "text": "Plan"},
            {"type": "tool_use", "id": "t1", "name": "TodoWrite",
             "input": {"todos": [{"content": "A", "status": "completed"}, {"content": "B", "status": "pending"}]}},
            {"type": "tool_use", "id": "t2", "name": "Write", "input": {"file_path": "a.txt", "content": "x"}}
        ]}}),
        json!({"type": "user", "message": {"content": [
            {"type": "tool_result", "tool_use_id": "t2", "content": [{"type": "text", "text": "ok"}]}]}}),
        json!({"type": "stream_event", "event": {"type": "content_block_delta",
               "delta": {"type": "text_delta", "text": "Go"}}}),
        json!({"type": "result", "subtype": "success", "is_error": false, "result": "Gotowe",
               "session_id": "s-1", "num_turns": 3, "duration_ms": 10, "total_cost_usd": 0.0123,
               "usage": {"input_tokens": 5, "output_tokens": 7, "cache_read_input_tokens": 2,
                         "cache_creation_input_tokens": 1}}),
    ]);
    assert!(matches!(&ev[0], AgentEvent::SessionStarted { session } if session.id == "s-1"));
    assert!(matches!(&ev[1], AgentEvent::Step { text } if text == "myślę"));
    assert!(
        ev.iter()
            .any(|e| matches!(e, AgentEvent::Plan { items } if items.len() == 2 && items[0].done))
    );
    assert!(ev.iter().any(|e| matches!(e, AgentEvent::FileChanged { path, change: FileChangeKind::Added } if path == Path::new("a.txt"))));
    assert!(
        ev.iter()
            .any(|e| matches!(e, AgentEvent::Output { partial: true, .. }))
    );
    assert!(ev.iter().any(|e| matches!(e, AgentEvent::Usage { cost_micro_usd: Some(12_300), usage } if usage.cache_write_tokens == 1)));
    let r = p.last_result().unwrap();
    assert_eq!(r.text, "Gotowe");
    assert_eq!(r.num_turns, Some(3));
    assert_eq!(r.session.as_ref().unwrap().workdir, Path::new("/wt/1"));
}

#[test]
fn tolerant_to_garbage_and_unknown_types() {
    let mut p = ClaudeParser::new(Path::new("/wt"));
    let garbage = p.line("to nie json");
    assert!(!garbage.json);
    assert!(matches!(&garbage.events[0], AgentEvent::Warning { .. }));
    let arr = p.line("[1,2]");
    assert!(
        arr.json
            && arr
                .events
                .iter()
                .all(|e| matches!(e, AgentEvent::Warning { .. }))
    );
    let unknown = p.line(r#"{"type":"nowy_typ","x":1}"#);
    assert!(
        matches!(&unknown.events[0], AgentEvent::Warning { message } if message.contains("nowy_typ"))
    );
    let bad_assistant = p.line(r#"{"type":"assistant","message":{"content":"tekst"}}"#);
    assert_eq!(bad_assistant.events.len(), 1);
    assert!(p.line(r#"{"type":"user"}"#).events.is_empty());
    assert!(
        p.line(r#"{"type":"system","subtype":"compact_boundary"}"#)
            .events
            .is_empty()
    );
    let init = p.line(r#"{"type":"system","subtype":"init","session_id":"s","mcp_servers":[{"name":"alfa","status":"failed"}]}"#);
    assert!(
        init.events
            .iter()
            .any(|e| matches!(e, AgentEvent::Warning { message } if message.contains("failed")))
    );
    let failed_tool = p.line(
        &json!({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "id": "e", "name": "Edit", "input": {"file_path": "b"}}]}})
        .to_string(),
    );
    assert_eq!(failed_tool.events.len(), 1);
    let res = p.line(
        &json!({"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "e", "is_error": true, "content": "zły"}]}})
        .to_string(),
    );
    assert!(
        res.events
            .iter()
            .all(|e| !matches!(e, AgentEvent::FileChanged { .. }))
    );
    assert!(p.line(r#"{"type":"result","is_error":true}"#).result);
    assert!(p.last_result().unwrap().is_error);
}

#[test]
fn micro_usd_rounding() {
    assert_eq!(micro_usd(Some(1.5)), Some(1_500_000));
    assert_eq!(micro_usd(Some(-1.0)), None);
    assert_eq!(micro_usd(Some(f64::NAN)), None);
    assert_eq!(micro_usd(None), None);
}
