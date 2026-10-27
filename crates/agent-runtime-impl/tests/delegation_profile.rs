//! Profil dostępu persony ogranicza również potomka w natywnym runtime.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use agent_runtime_contract::{DelegateArgs, RunBudget, RunId, RunOptions};
use agent_runtime_impl::{ParentView, plan_delegation};
use common::v1::{spec_for, standard_crew};
use personas_contract::{PersonaId, builtin_roles};
use tools_fs_contract::{FsToolKind, manifest};
#[test]
fn per_persona_profile_limits_native_delegation() {
    let spec = spec_for("alfa", "conductor", &["fs_read", "fs_write"]);
    let mut opts = RunOptions {
        crew: Some(standard_crew(&[])),
        ..RunOptions::default()
    };
    let mut role = builtin_roles()
        .into_iter()
        .find(|r| r.id.as_str() == "operator")
        .unwrap();
    role.tools.clear();
    role.read_only = true;
    opts.crew
        .as_mut()
        .unwrap()
        .persona_roles
        .insert(PersonaId::new("delta"), vec![role]);
    let run = RunId::new("profile-test");
    let view = ParentView {
        run: &run,
        spec: &spec,
        options: &opts,
        tools: vec![manifest(FsToolKind::Read), manifest(FsToolKind::Write)],
        remaining: RunBudget::default(),
        taint: None,
        autonomy: (None, None),
        provenance: ("", ""),
    };
    let args = DelegateArgs {
        role: "operator".into(),
        persona: Some("delta".into()),
        goal: "Read a file".into(),
        tools: None,
        max_steps: None,
    };
    let plan = plan_delegation(&view, &args, 3).unwrap();
    assert!(plan.spec.tools.is_empty());
    assert!(
        plan.spec
            .roles
            .iter()
            .all(|r| r.tools.is_empty() && r.read_only)
    );
}
