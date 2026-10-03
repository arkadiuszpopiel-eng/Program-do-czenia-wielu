//! Komendy poza czatem: Hub kont (sekret nigdy nie wraca), start UI, ustawienia, skróty,
//! obsada, Szybkie pytanie, urządzenie, rejestr modułów, trwałość po restarcie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use accounts_hub_contract::SecretStore;
use app_core::dto::{
    AccountAssignment, AddAccountInput, CastTemplateId, LayoutPrefs, Money, PanelId, SessionPanels,
    SessionTemplate, SettingValue, TurnErrorCode,
};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths, MemorySecretStore};
use common::*;
use core_registry_contract::ModuleState;

const KEY: &str = "sk-test-NIE-POKAZUJ-0123456789";

fn add_input(provider: &str, base_url: Option<&str>) -> AddAccountInput {
    serde_json::from_value(serde_json::json!({
        "provider_id": provider,
        "label": "",
        "secret": KEY,
        "base_url": base_url,
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accounts_store_key_in_vault_and_never_return_it() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let shell = Arc::new(HeadlessShell::default());
    let mut opts = options(None, shell);
    opts.secrets = Some(secrets.clone());
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut rx = core.subscribe_events();

    assert_eq!(core.accounts_catalog().await.unwrap().len(), 17);
    assert!(!core.system_status().await.unwrap().keys_configured);
    let account = core
        .accounts_add(add_input(
            "custom-openai-compatible",
            Some("http://127.0.0.1:9/v1"),
        ))
        .await
        .unwrap();
    assert!(account.key_stored);
    assert_eq!(account.label, "Własny endpoint (zgodny z OpenAI)");
    let listed = serde_json::to_string(&core.accounts_list().await.unwrap()).unwrap();
    assert!(!listed.contains(KEY), "sekret wrócił do UI");
    let stored = secrets.list().unwrap();
    assert!(
        stored
            .iter()
            .any(|n| n.as_str() == format!("accounts/{}", account.id))
    );
    assert!(
        !std::fs::read_to_string(dir.path().join("config/accounts.json"))
            .unwrap()
            .contains(KEY)
    );
    assert!(core.system_status().await.unwrap().keys_configured);

    // Bez sieci: wysłanie → błąd dostawcy/offline (bez paniki); test konta → czytelny błąd,
    // a konto w stanie błędu przestaje być „mózgiem" (brak kluczy).
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let turn = core
        .turns_send(sid, send("Cześć", None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    let events = until(&mut rx, ends(&turn)).await;
    let error = events.iter().find_map(|e| match e {
        app_core::dto::AlfaEvent::Error { error, .. } => Some(error.code),
        _ => None,
    });
    assert!(
        matches!(
            error,
            Some(TurnErrorCode::Provider | TurnErrorCode::Offline)
        ),
        "{error:?}"
    );
    let report = core.accounts_test(account.id.clone()).await.unwrap();
    assert!(!report.ok);
    assert!(report.error.is_some());
    assert!(!core.system_status().await.unwrap().keys_configured);

    core.accounts_assign(
        account.id.clone(),
        AccountAssignment {
            task_classes: vec!["chat".into(), "code".into()],
            agents: vec!["beta".into()],
            voice_stt: true,
            voice_tts: false,
        },
    )
    .await
    .unwrap();
    core.accounts_set_limit(account.id.clone(), true, Money::pln(5_000))
        .await
        .unwrap();
    let after = core.accounts_list().await.unwrap().remove(0);
    assert_eq!(after.assignments.task_classes, vec!["chat", "code"]);
    assert_eq!(after.assignments.agents, vec!["beta"]);
    assert!(after.assignments.voice_stt);
    assert_eq!(after.cost_limit.monthly, Money::pln(5_000));
    core.accounts_remove(account.id).await.unwrap();
    assert!(
        secrets
            .list()
            .unwrap()
            .iter()
            .all(|n| !n.as_str().starts_with("accounts/"))
    );
    assert!(!core.system_status().await.unwrap().keys_configured);
    let blank: AddAccountInput = serde_json::from_value(serde_json::json!({
        "provider_id": "openai", "label": "x", "secret": "  ", "base_url": null
    }))
    .unwrap();
    assert!(core.accounts_add(blank).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bootstrap_layout_settings_shortcuts_and_cast() {
    let h = harness().await;
    let core = &h.core;
    let boot = core.app_bootstrap().await.unwrap();
    assert!(!boot.onboarding_done);
    assert_eq!(
        boot.settings.get("ui.theme"),
        Some(&SettingValue::Text("auto".into()))
    );
    core.app_complete_onboarding().await.unwrap();
    let sid = core
        .sessions_create(SessionTemplate::Research)
        .await
        .unwrap()
        .id;
    let layout = LayoutPrefs {
        left_width: 260,
        right_width: 380,
        left_collapsed: false,
        sessions: [(
            sid.clone(),
            SessionPanels {
                left_open: true,
                right_open: true,
                right_tab: PanelId::Timeline,
            },
        )]
        .into(),
    };
    core.app_save_layout(layout.clone()).await.unwrap();
    core.app_set_active_session(Some(sid.clone()))
        .await
        .unwrap();
    core.settings_set("ui.theme".into(), SettingValue::Text("dark".into()))
        .await
        .unwrap();
    core.settings_set(
        "general.destroy_webview_after".into(),
        SettingValue::Number(15.into()),
    )
    .await
    .unwrap();
    core.settings_set_shortcut("palette.open".into(), Some("Ctrl+Shift+P".into()))
        .await
        .unwrap();
    core.settings_set_shortcut("focus.toggle".into(), Some(String::new()))
        .await
        .unwrap();
    let boot = core.app_bootstrap().await.unwrap();
    assert!(boot.onboarding_done);
    assert_eq!(boot.layout, Some(layout));
    assert_eq!(boot.active_session_id.as_deref(), Some(sid.as_str()));
    assert_eq!(
        boot.settings.get("ui.theme"),
        Some(&SettingValue::Text("dark".into()))
    );
    assert_eq!(
        core.setting("general.destroy_webview_after").await,
        Some(SettingValue::Number(15.into()))
    );
    assert_eq!(
        boot.shortcut_overrides
            .get("palette.open")
            .map(String::as_str),
        Some("Ctrl+Shift+P")
    );
    assert_eq!(
        boot.shortcut_overrides
            .get("focus.toggle")
            .map(String::as_str),
        Some("")
    );
    assert_eq!(
        core.settings_reset("ui.theme".into()).await.unwrap(),
        SettingValue::Text("auto".into())
    );
    assert_eq!(core.settings_schema().await.unwrap().len(), 27);

    // Obsada: identyfikatory ról jak w `personas-contract` (`operator`), szablony.
    let agents = core.agents_list(sid.clone()).await.unwrap();
    assert_eq!(agents.len(), 4);
    let alfa = agents.iter().find(|a| a.id == "alfa").unwrap();
    assert!(alfa.role_ids.contains(&"conductor".to_owned()));
    core.agents_set_roles(
        sid.clone(),
        "delta".into(),
        vec!["operator".into(), "coder".into()],
    )
    .await
    .unwrap();
    let delta = core.agents_list(sid.clone()).await.unwrap();
    let delta = delta.iter().find(|a| a.id == "delta").unwrap();
    assert!(delta.role_ids.contains(&"operator".to_owned()));
    core.agents_apply_cast(sid.clone(), CastTemplateId::Solo)
        .await
        .unwrap();
    let solo = core.agents_list(sid.clone()).await.unwrap();
    assert!(
        solo.iter()
            .filter(|a| a.id != "alfa")
            .all(|a| a.role_ids.is_empty())
    );
    core.agents_apply_cast(sid.clone(), CastTemplateId::Standard)
        .await
        .unwrap();
    assert!(
        core.agents_set_roles(sid, "omega".into(), vec![])
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quick_ask_device_registry_and_restart_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(HeadlessShell::default());
    let mut opts = options(Some(provider.clone()), shell.clone());
    opts.secrets = Some(secrets.clone());
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut rx = core.subscribe_events();

    let first = core
        .quick_ask("Ile to 17% z 2400 zł?".into())
        .await
        .unwrap();
    until(&mut rx, ends(first.assistant_turn_id.as_deref().unwrap())).await;
    let second = core.quick_ask("A 23%?".into()).await.unwrap();
    until(&mut rx, ends(second.assistant_turn_id.as_deref().unwrap())).await;
    assert_eq!(first.session_id, second.session_id, "tryb jednej sesji");
    core.quick_expand_to_main(first.session_id.clone())
        .await
        .unwrap();
    core.quick_hide().await.unwrap();
    let calls = shell.calls();
    assert!(calls.contains(&format!("show_main:{}", first.session_id)));
    assert!(calls.iter().any(|c| c == "hide_quick"));

    let device = core.device_profile().await.unwrap();
    assert_eq!(device.machine.id.len(), 32);
    assert_eq!(
        core.device_measure().await.unwrap().machine.id,
        device.machine.id
    );

    let modules = core.modules().await;
    assert!(modules.len() >= 12);
    assert!(
        modules.iter().all(|m| m.state == ModuleState::Ready),
        "{modules:#?}"
    );
    assert_eq!(
        core.provider_of("sessions-contract").await.as_deref(),
        Some("sessions")
    );
    let draft_sid = first.session_id.clone();
    core.sessions_save_draft(draft_sid.clone(), "szkic".into())
        .await
        .unwrap();
    drop(core);
    drop(rx);

    // Restart na tych samych katalogach i tym samym sejfie: sesja, tury i szkic wracają.
    let mut opts = options(Some(provider), shell);
    opts.secrets = Some(secrets);
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let snap = core.turns_list(draft_sid.clone()).await.unwrap();
    assert_eq!(snap.turns.len(), 4);
    assert!(
        snap.turns
            .iter()
            .all(|t| t.usage.is_some() || t.author == "user")
    );
    assert_eq!(core.sessions_get_draft(draft_sid).await.unwrap(), "szkic");
    let boot = core.app_bootstrap().await.unwrap();
    assert_eq!(
        boot.active_session_id.as_deref(),
        Some(first.session_id.as_str())
    );
}
