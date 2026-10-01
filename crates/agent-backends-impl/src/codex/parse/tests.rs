use super::*;

#[test]
fn notifications_map_to_events() {
    let mut st = CodexState::default();
    let (ev, end) = notification("item/agentMessage/delta", &json!({"delta": "Go"}), &mut st);
    assert!(matches!(&ev[0], AgentEvent::Output { partial: true, .. }) && end.is_none());
    let (ev, _) = notification(
        "item/started",
        &json!({"item": {"type": "commandExecution", "id": "c1", "command": "ls", "cwd": "/wt"}}),
        &mut st,
    );
    assert!(matches!(&ev[0], AgentEvent::ToolRequest { tool, .. } if tool == "commandExecution"));
    let (ev, _) = notification(
        "item/completed",
        &json!({"item": {"type": "commandExecution", "id": "c1", "status": "completed", "exitCode": 1,
                         "aggregatedOutput": "x"}}),
        &mut st,
    );
    assert!(matches!(
        &ev[0],
        AgentEvent::ToolFinished { is_error: true, .. }
    ));
    let (ev, _) = notification(
        "item/completed",
        &json!({"item": {"type": "fileChange", "id": "f", "status": "completed",
                         "changes": [{"path": "a", "kind": "add"}, {"path": "b", "kind": {"type": "delete"}},
                                     {"path": "c", "kind": "update"}]}}),
        &mut st,
    );
    let kinds: Vec<_> = ev
        .iter()
        .filter_map(|e| match e {
            AgentEvent::FileChanged { change, .. } => Some(*change),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        [
            FileChangeKind::Added,
            FileChangeKind::Deleted,
            FileChangeKind::Modified
        ]
    );
    let (ev, _) = notification(
        "item/completed",
        &json!({"item": {"type": "agentMessage", "id": "m", "text": "Gotowe"}}),
        &mut st,
    );
    assert_eq!(ev.len(), 1);
    assert_eq!(st.last_message, "Gotowe");
    let (ev, _) = notification(
        "item/completed",
        &json!({"item": {"type": "reasoning", "id": "r", "summary": ["a", "b"]}}),
        &mut st,
    );
    assert!(matches!(&ev[0], AgentEvent::Step { text } if text == "a\nb"));
    let (ev, _) = notification(
        "turn/plan/updated",
        &json!({"plan": [{"step": "A", "status": "completed"}, {"step": "B", "status": "pending"}]}),
        &mut st,
    );
    assert!(matches!(&ev[0], AgentEvent::Plan { items } if items[0].done && !items[1].done));
    let (ev, _) = notification(
        "thread/tokenUsage/updated",
        &json!({"tokenUsage": {"total": {"inputTokens": 10, "cachedInputTokens": 4, "outputTokens": 3}}}),
        &mut st,
    );
    assert!(
        matches!(&ev[0], AgentEvent::Usage { usage, .. } if usage.input_tokens == 6 && usage.cache_read_tokens == 4)
    );
    let (_, end) = notification(
        "turn/completed",
        &json!({"turn": {"id": "t", "status": "failed", "error": {"message": "limit"}}}),
        &mut st,
    );
    assert_eq!(
        end,
        Some(TurnEnd {
            status: "failed".into(),
            error: Some("limit".into())
        })
    );
    let (ev, _) = notification("error", &json!({"error": {"message": "x"}}), &mut st);
    assert!(matches!(&ev[0], AgentEvent::Warning { .. }));
    assert!(
        notification("account/updated", &json!({}), &mut st)
            .0
            .is_empty()
    );
    assert!(
        notification(
            "item/started",
            &json!({"item": {"type": "userMessage"}}),
            &mut st
        )
        .0
        .is_empty()
    );
    let (ev, _) = notification(
        "item/started",
        &json!({"item": {"type": "mcpToolCall", "id": "m", "server": "s", "tool": "t", "arguments": {}}}),
        &mut st,
    );
    assert!(matches!(&ev[0], AgentEvent::ToolRequest { tool, .. } if tool == "mcp:s/t"));
}

#[test]
fn approvals_both_protocol_generations() {
    let a = approval_request(
        "item/commandExecution/requestApproval",
        &json!({"itemId": "i1", "command": "rm -rf x", "cwd": "/wt", "reason": "sprzątanie"}),
    )
    .unwrap();
    assert_eq!(
        (a.kind, a.call_id.as_deref(), a.reason.as_deref()),
        (PermissionKind::Command, Some("i1"), Some("sprzątanie"))
    );
    let f = approval_request("item/fileChange/requestApproval", &json!({"itemId": "i2"})).unwrap();
    assert_eq!(f.kind, PermissionKind::FileChange);
    assert!(approval_request("execCommandApproval", &json!({"callId": "c"})).is_some());
    assert!(
        approval_request("applyPatchApproval", &json!({}))
            .unwrap()
            .call_id
            .is_none()
    );
    assert!(approval_request("sampling/createMessage", &json!({})).is_none());
    let allow = ApprovalDecision::allow();
    let deny = ApprovalDecision::deny("nie");
    assert_eq!(
        approval_response("item/fileChange/requestApproval", &allow),
        json!({"decision": "accept"})
    );
    assert_eq!(
        approval_response("item/commandExecution/requestApproval", &deny),
        json!({"decision": "decline"})
    );
    assert_eq!(
        approval_response("execCommandApproval", &allow),
        json!({"decision": "approved"})
    );
    assert_eq!(
        approval_response("applyPatchApproval", &deny),
        json!({"decision": "denied"})
    );
}
