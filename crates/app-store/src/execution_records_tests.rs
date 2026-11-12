//! Syntetyczne bazy testowe; żadne narzędzie komputera nie jest tu wykonywane.
use crate::{AppStore, RecordOrigin, RecordedState, TurnMeta};
use app_api::{dto::*, ids};
use lib_sqlstore::rusqlite::params;
use sessions_contract::{SessionId, TurnId};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[path = "execution_records_test_fixture.rs"]
mod fixture;
use fixture::{Fixture, Provider, SECRET, step};

#[test]
fn fresh_selection_does_not_open_storage_and_storage_failure_is_not_zero_facts() {
    let store = AppStore::new(Arc::new(Provider(BTreeMap::new())));
    let sid = SessionId::new("not-opened");
    assert!(
        store
            .execution_records(&sid, &[], &BTreeSet::new())
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .execution_records(&sid, &[TurnId(1)], &BTreeSet::new())
            .is_err()
    );
}

#[test]
fn native_replay_is_bound_to_checkpoint_and_releases_no_private_payload() {
    let f = Fixture::new();
    let id = f.replay("private-run-id", 2, ReplayStatus::Ok, None, true);
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    let records = f.records(&[2]);
    assert_eq!(records.counts.recorded_done, 1);
    assert_eq!(records.records[0].tool.as_deref(), Some("fs_write"));
    assert_eq!(records.records[0].origin, RecordOrigin::NativeReplay);
    let json = serde_json::to_string(&records).unwrap();
    for hidden in [
        SECRET,
        "private-run-id",
        &id,
        "undo_token",
        "command",
        "input",
        "output",
    ] {
        assert!(!json.contains(hidden), "wyciek {hidden}");
    }
    assert_eq!(
        f.store.runs(&f.sid).unwrap().len(),
        1,
        "odczyt nie tworzy nowego przebiegu"
    );
}

#[test]
fn a_replay_header_without_runtime_checkpoint_does_not_assert_native_origin() {
    let f = Fixture::new();
    let id = f.replay("legacy", 2, ReplayStatus::Ok, None, false);
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    let r = f.records(&[2]);
    assert_eq!(r.records[0].origin, RecordOrigin::TurnMetadataOnly);
    assert!(r.records[0].tool.is_none());
    assert_eq!(r.records[0].recorded_status, ToolStatus::Done);
}

#[test]
fn cli_claim_is_unverified_even_with_an_undo_claim_and_runtime_row() {
    let f = Fixture::new();
    let id = f.replay("cli", 2, ReplayStatus::Ok, Some("codex"), true);
    let mut tool = step(&id, ToolStatus::Done);
    tool.undone = true;
    f.meta(2, vec![tool]);
    let r = f.records(&[2]);
    assert_eq!(r.counts.unverified, 1);
    assert_eq!(r.counts.recorded_done, 0);
    assert_eq!(r.records[0].origin, RecordOrigin::UnverifiedCli);
    assert!(r.records[0].tool.is_none());
}

#[test]
fn latest_replay_status_wins_instead_of_an_earlier_success() {
    let f = Fixture::new();
    let id = f.replay("r", 2, ReplayStatus::Ok, None, true);
    f.replay("r", 2, ReplayStatus::NeedsConfirmation, None, true);
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    let r = f.records(&[2]);
    assert_eq!(r.counts.unverified, 1);
    assert_eq!(r.counts.recorded_done, 0);
}

#[test]
fn other_branch_owners_are_not_trusted_and_other_turns_are_not_counted() {
    let f = Fixture::new();
    let id = f.replay("foreign", 99, ReplayStatus::Ok, None, true);
    f.meta(99, vec![step("other-action", ToolStatus::Done)]);
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    let r = f.records(&[2]);
    assert_eq!(r.counts.unverified, 1);
    assert_eq!(r.records.len(), 1);
    assert!(r.records[0].tool.is_none());
}

#[test]
fn proposals_errors_running_undo_and_legacy_intents_keep_distinct_states() {
    let f = Fixture::new();
    let mut undone = step("undone", ToolStatus::Done);
    undone.undo_token = Some("private-undo-token".into());
    let mut intent = step("legacy", ToolStatus::Done);
    intent.intent = Some(ToolIntent {
        kind: IntentKind::OpenInTerminal,
        title: SECRET.into(),
        command: Some(SECRET.into()),
        cwd: Some(SECRET.into()),
        shell: Some(SECRET.into()),
        paths: vec![SECRET.into()],
    });
    f.meta(
        2,
        vec![
            step("pending", ToolStatus::NeedsConfirmation),
            step("error", ToolStatus::Error),
            step("running", ToolStatus::Running),
            undone,
            intent,
        ],
    );
    f.store.push_undone(&f.sid, "private-undo-token").unwrap();
    let r = f.records(&[2]);
    assert_eq!(r.counts.needs_confirmation, 1);
    assert_eq!(r.counts.error_or_denied, 1);
    assert_eq!(r.counts.recorded_running, 1);
    assert_eq!(r.counts.undone, 1);
    assert_eq!(r.counts.unverified, 1);
    assert_eq!(r.counts.recorded_done, 0);
    assert!(r.records_truncated);
    assert!(!serde_json::to_string(&r).unwrap().contains(SECRET));
}

#[test]
fn missing_corrupt_or_legacy_metadata_is_unknown_and_integer_overflow_does_not_alias() {
    let f = Fixture::new();
    f.store
        .with(&f.sid, |db| {
            db.execute("INSERT INTO app_turn_meta VALUES(?1,?2)", params![2, "{"])
                .map(|_| ())
        })
        .unwrap();
    f.store
        .with(&f.sid, |db| {
            db.execute(
                "INSERT INTO app_turn_meta VALUES(?1,?2)",
                params![3, r#"{"agent":"alfa"}"#],
            )
            .map(|_| ())
        })
        .unwrap();
    f.meta(
        i64::MAX as u64,
        vec![step("must-not-alias", ToolStatus::Done)],
    );
    let r = f.records(&[1, 2, 3, 0, u64::MAX]);
    assert_eq!(r.missing_metadata_turns, 5);
    assert_eq!(r.counts.recorded_done, 0);
    assert!(r.records.is_empty());
}

#[test]
fn duplicate_step_ids_use_the_latest_state_while_missing_ids_stay_separate() {
    let f = Fixture::new();
    f.meta(1, vec![step("same", ToolStatus::Running)]);
    f.meta(
        2,
        vec![
            step("same", ToolStatus::Done),
            step("", ToolStatus::Done),
            step("", ToolStatus::Done),
        ],
    );
    let r = f.records(&[1, 2, 2]);
    assert_eq!(r.counts.recorded_done, 1);
    assert_eq!(r.counts.recorded_running, 0);
    assert_eq!(r.counts.unverified, 2);
    assert_eq!(r.records.len(), 3);
}

#[test]
fn more_than_four_actions_preserve_all_counts_inside_the_wire_budget() {
    let f = Fixture::new();
    f.meta(
        2,
        (0..100)
            .map(|n| step(&format!("s{n}"), ToolStatus::Done))
            .collect(),
    );
    let r = f.records(&[2]);
    assert_eq!(r.counts.recorded_done, 100);
    assert!(r.records_truncated);
    assert!(r.records.len() <= 4);
    assert!(serde_json::to_vec(&r).unwrap().len() <= 1536);
}

#[test]
fn a_second_session_with_the_same_numeric_turn_id_does_not_leak_into_the_first() {
    let f = Fixture::new();
    f.store
        .put_meta(
            &SessionId::new("other"),
            TurnId(2),
            &TurnMeta {
                tools: vec![step("other-private-action", ToolStatus::Done)],
                ..Default::default()
            },
        )
        .unwrap();
    f.meta(2, vec![]);
    let r = f.records(&[2]);
    assert_eq!(r.counts.recorded_done, 0);
    assert_eq!(r.missing_metadata_turns, 0);
}

#[test]
fn malformed_latest_replay_does_not_fall_back_to_a_good_older_record() {
    let f = Fixture::new();
    let id = f.replay("r", 2, ReplayStatus::Ok, None, true);
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    f.store
        .with(&f.sid, |db| {
            db.execute(
                "INSERT INTO app_agent_steps(run_id,step_id,body) VALUES(?1,?2,?3)",
                params![ids::run_dto(&f.sid, "r"), id, "{"],
            )
            .map(|_| ())
        })
        .unwrap();
    assert_eq!(f.records(&[2]).records[0].state, RecordedState::Unverified);
}

#[test]
fn unknown_tool_names_are_hidden_without_turning_a_record_into_a_business_action() {
    let f = Fixture::new();
    let id = f.replay("r", 2, ReplayStatus::Ok, None, true);
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    let r = f
        .store
        .execution_records(&f.sid, &[TurnId(2)], &BTreeSet::new())
        .unwrap()
        .unwrap();
    assert!(r.records[0].tool.is_none());
    assert!(r.lookup_is_not_agent_action);
}

#[test]
fn tool_actor_comes_from_native_replay_instead_of_the_parent_answer_author() {
    let f = Fixture::new();
    let id = f.replay("r", 2, ReplayStatus::Ok, None, true);
    f.set_header("r", "delta", None);
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    assert_eq!(f.records(&[2]).records[0].agent.as_deref(), Some("delta"));
}

#[test]
fn latest_bridge_header_overrides_an_older_native_header() {
    let f = Fixture::new();
    let id = f.replay("r", 2, ReplayStatus::Ok, None, true);
    f.set_header("r", "alfa", Some("codex"));
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    assert_eq!(
        f.records(&[2]).records[0].origin,
        RecordOrigin::UnverifiedCli
    );
}

#[test]
fn a_model_verification_step_is_not_mistaken_for_a_business_tool() {
    let f = Fixture::new();
    let id = f.replay("r", 2, ReplayStatus::Ok, None, true);
    f.set_step("r", "fs_write", ReplayKind::Verify);
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    assert_eq!(f.records(&[2]).counts.unverified, 1);
}

#[test]
fn latest_step_with_the_wrong_run_column_is_not_skipped_for_an_older_success() {
    let f = Fixture::new();
    let id = f.replay("r", 2, ReplayStatus::Ok, None, true);
    f.meta(2, vec![step(&id, ToolStatus::Done)]);
    f.store
        .with(&f.sid, |db| {
            db.execute(
                "INSERT INTO app_agent_steps(run_id,step_id,body) VALUES(?1,?2,?3)",
                params!["unrelated-run", id, SECRET],
            )
            .map(|_| ())
        })
        .unwrap();
    assert_eq!(f.records(&[2]).counts.unverified, 1);
}

#[test]
fn body_identity_and_ordinal_must_match_the_selected_step_columns() {
    for wrong in [
        serde_json::json!({"id":"foreign:s1"}),
        serde_json::json!({"n":99}),
    ] {
        let f = Fixture::new();
        let id = f.replay("r", 2, ReplayStatus::Ok, None, true);
        f.meta(2, vec![step(&id, ToolStatus::Done)]);
        f.store
            .with(&f.sid, |db| {
                let body: String = db.query_row(
                    "SELECT body FROM app_agent_steps ORDER BY seq DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )?;
                let mut body: serde_json::Value = serde_json::from_str(&body).unwrap();
                for (key, value) in wrong.as_object().unwrap() {
                    body[key] = value.clone();
                }
                db.execute(
                    "INSERT INTO app_agent_steps(run_id,step_id,body) VALUES(?1,?2,?3)",
                    params![ids::run_dto(&f.sid, "r"), id, body.to_string()],
                )
                .map(|_| ())
            })
            .unwrap();
        assert_eq!(f.records(&[2]).counts.unverified, 1);
    }
}

#[test]
fn maximum_length_names_fit_the_wire_budget_without_losing_the_complete_counts() {
    let f = Fixture::new();
    let name = "t".repeat(64);
    let actor = "a".repeat(64);
    let mut tools = Vec::new();
    for n in 0..4 {
        let raw = format!("r{n}");
        let id = f.replay(&raw, 2, ReplayStatus::NeedsConfirmation, None, true);
        f.set_header(&raw, &actor, None);
        f.set_step(&raw, &name, ReplayKind::Tool);
        tools.push(step(&id, ToolStatus::NeedsConfirmation));
    }
    f.meta(2, tools);
    let r = f
        .store
        .execution_records(&f.sid, &[TurnId(2)], &BTreeSet::from([name]))
        .unwrap()
        .unwrap();
    assert_eq!(r.counts.needs_confirmation, 4);
    assert_eq!(r.records.len(), 4);
    assert!(
        r.records
            .iter()
            .all(|r| r.tool.as_deref() == Some("t".repeat(64).as_str()))
    );
    assert!(serde_json::to_vec(&r).unwrap().len() <= 1536);
}
