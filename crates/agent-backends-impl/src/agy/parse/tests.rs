use super::*;

fn parse_all(lines: &[&str]) -> (AgyParser, Vec<AgentEvent>) {
    let mut p = AgyParser::new(Path::new("/wt"));
    let mut events = Vec::new();
    for l in lines {
        events.extend(p.line(l).events);
    }
    (p, events)
}

#[test]
fn full_flow_maps_steps_to_events() {
    let (p, events) = parse_all(&[
        r#"{"type":"init","conversation_id":"conv-1","model":"gemini"}"#,
        r#"{"type":"step_update","step_id":"p","step_type":"plan","items":[{"text":"Plik","done":true},{"text":"Podsumuj","status":"running"}]}"#,
        r#"{"type":"step_update","step_id":"t1","step_type":"tool","status":"running","tool":{"name":"write_file","input":{"file_path":"wynik.txt","content":"ok"}}}"#,
        r#"{"type":"step_update","step_id":"t1","step_type":"tool","status":"done","output":"zapisano"}"#,
        r#"{"type":"step_update","step_id":"r","step_type":"agent_response","status":"running","text_delta":"Got"}"#,
        r#"{"type":"step_update","step_id":"r","step_type":"agent_response","status":"done","text_delta":"owe."}"#,
        r#"{"type":"result","status":"SUCCESS","response":"Gotowe.","usage":{"input_tokens":12,"output_tokens":4},"duration_ms":30}"#,
    ]);
    assert!(
        matches!(&events[0], AgentEvent::SessionStarted { session } if session.id == "conv-1" && session.bridge == BridgeKind::Antigravity)
    );
    assert!(
        matches!(&events[1], AgentEvent::Plan { items } if items.len() == 2 && items[0].done && !items[1].done)
    );
    assert!(
        matches!(&events[2], AgentEvent::ToolRequest { call_id, tool, .. } if call_id == "t1" && tool == "write_file")
    );
    assert!(
        matches!(&events[3], AgentEvent::FileChanged { path, change: FileChangeKind::Added } if path == Path::new("wynik.txt"))
    );
    assert!(matches!(
        &events[4],
        AgentEvent::ToolFinished {
            is_error: false,
            ..
        }
    ));
    let full: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Output {
                text,
                partial: false,
                ..
            } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(full, ["Gotowe."], "{events:?}");
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Usage { usage, cost_micro_usd: None } if usage.input_tokens == 12)));
    let r = p.last_result().unwrap();
    assert_eq!(r.text, "Gotowe.");
    assert!(!r.is_error);
    assert_eq!(r.session.as_ref().unwrap().id, "conv-1");
    assert_eq!(r.duration_ms, Some(30));
}

#[test]
fn denied_tool_has_no_file_change_and_error_result() {
    let (p, events) = parse_all(&[
        r#"{"type":"step_update","conversation_id":"c2","step_id":"t","step_type":"tool","tool":{"name":"run_command","input":{"command":"rm -rf x"}}}"#,
        r#"{"type":"step_update","step_id":"t","step_type":"tool","status":"denied","message":"wymaga zgody"}"#,
        r#"{"type":"result","status":"ERROR","response":"przerwano"}"#,
    ]);
    assert!(matches!(&events[0], AgentEvent::SessionStarted { session } if session.id == "c2"));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::FileChanged { .. }))
    );
    assert!(events.iter().any(
        |e| matches!(e, AgentEvent::ToolFinished { is_error: true, preview, .. } if preview.contains("zgody"))
    ));
    let r = p.last_result().unwrap();
    assert!(r.is_error);
    assert_eq!(r.subtype.as_deref(), Some("error"));
}

#[test]
fn garbage_and_unknown_types_warn_without_failing() {
    let mut p = AgyParser::new(Path::new("/wt"));
    let bad = p.line("to nie jest json");
    assert!(!bad.json);
    assert!(matches!(&bad.events[0], AgentEvent::Warning { .. }));
    let unknown = p.line(r#"{"type":"telemetria","x":1}"#);
    assert!(unknown.json && matches!(&unknown.events[0], AgentEvent::Warning { .. }));
    assert!(
        p.line(r#"{"type":"step_update","step_type":"nowy"}"#)
            .events
            .is_empty()
    );
    assert!(p.line(r#"{"type":"result"}"#).result);
    assert!(!p.last_result().unwrap().is_error, "brak statusu = sukces");
}
