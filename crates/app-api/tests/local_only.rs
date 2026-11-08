//! B.5: tabela poleceń `local_only` jest spójna z listą komend i schematem łącza — wpis do nieistniejącego
//! polecenia, odpowiednik, który sam wymaga lokalnej maszyny, albo pole ścieżki, którego nie ma w DTO,
//! oznacza, że tabela się zestarzała.

use app_api::local_only::{LocalReason, Remote, is_local_only, local_only};
use app_api::schema::ipc_schema;
use app_api::{CHANNEL_COMMANDS, COMMANDS};
use serde_json::Value;

fn known(command: &str) -> bool {
    COMMANDS.contains(&command) || CHANNEL_COMMANDS.contains(&command)
}

#[test]
fn table_parses_and_names_real_commands() {
    let table = local_only();
    assert!(table.commands.len() >= 25, "{}", table.commands.len());
    for (name, entry) in &table.commands {
        assert!(known(name), "nieznane polecenie {name}");
        match &entry.remote {
            Remote::Command { command } => {
                assert!(known(command), "{name}: odpowiednik {command} nie istnieje");
                assert!(
                    !is_local_only(command),
                    "{name}: odpowiednik {command} też lokalny"
                );
            }
            Remote::Planned { planned, how } => {
                assert!(planned.starts_with("D."), "{name}: {planned}");
                assert!(!how.trim().is_empty(), "{name}");
            }
            Remote::None { none } => assert!(!none.trim().is_empty(), "{name}"),
        }
    }
    assert!(is_local_only("projects_pick_dir"));
    assert!(!is_local_only("turns_send"));
}

/// Polecenie, którego nazwa mówi o oknie systemu albo pliku hosta, musi mieć wpis — nowe polecenie
/// tego rodzaju nie przejdzie po cichu do klientów zdalnych.
#[test]
fn dialog_like_commands_are_marked() {
    let hints = [
        "_pick",
        "pick_",
        "choose_dir",
        "_export",
        "_import",
        "open_",
        "_restart",
    ];
    let table = local_only();
    for name in COMMANDS {
        if hints.iter().any(|h| name.contains(h)) {
            assert!(
                table.commands.contains_key(*name),
                "{name} bez wpisu local_only"
            );
        }
    }
    let dialogs = table
        .commands
        .values()
        .filter(|c| c.reason == LocalReason::SystemDialog)
        .count();
    assert!(dialogs >= 10, "{dialogs}");
}

fn has_field(defs: &Value, ty: &str, field: &str) -> bool {
    let Some(def) = defs.get(ty) else {
        return false;
    };
    let direct = def.pointer(&format!("/properties/{field}")).is_some();
    let variants = ["oneOf", "anyOf"].iter().any(|k| {
        def.get(*k).and_then(Value::as_array).is_some_and(|vs| {
            vs.iter()
                .any(|v| v.pointer(&format!("/properties/{field}")).is_some())
        })
    });
    direct || variants
}

#[test]
fn host_path_fields_exist_and_table_is_in_schema() {
    let schema = ipc_schema();
    let defs = &schema["$defs"];
    for path in &local_only().host_paths {
        let (ty, field) = path.split_once('.').unwrap_or((path, ""));
        assert!(has_field(defs, ty, field), "brak pola {path} w schemacie");
    }
    let listed = schema["local_only"]["commands"]
        .as_object()
        .map(|m| m.len());
    assert_eq!(listed, Some(local_only().commands.len()));
}
