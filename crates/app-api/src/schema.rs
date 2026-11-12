//! Schemat JSON łącza rdzeń ↔ klient, wyprowadzony z typów Rust (ADR 0013; plan v6, zadanie
//! B.1): każda komenda z `with_commands!` (argumenty i wynik), ramka zdarzeń, błąd komendy i
//! wszystkie DTO, do których sięgają. Z tego pliku `scripts/gen-ipc-types.mjs` generuje typy
//! TypeScript; test `tests/ipc_schema.rs` pilnuje, żeby zapisany schemat
//! (`packages/schemas/ipc.v1.json`) nie rozjechał się z kodem.

use schemars::SchemaGenerator;
use schemars::generate::SchemaSettings;
use serde_json::{Map, Value, json};

use crate::dto::{EventFrame, EventsResume, TerminalFrame};
use crate::error::AppError;

/// Wersja kształtu pliku schematu (nie wersja programu).
pub const IPC_SCHEMA_VERSION: u32 = 1;

macro_rules! command_schemas {
    ($( $name:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty ; )*) => {
        fn commands(generator: &mut SchemaGenerator) -> Map<String, Value> {
            let mut all = Map::new();
            $(
                // Kolejność argumentów jest częścią kontraktu — lista, nie słownik.
                let args: Vec<Value> = vec![
                    $( json!({
                        "name": stringify!($arg),
                        "schema": generator.subschema_for::<$ty>(),
                    }), )*
                ];
                all.insert(
                    stringify!($name).to_owned(),
                    json!({ "args": args, "result": generator.subschema_for::<$ret>() }),
                );
            )*
            all
        }
    };
}

crate::with_commands!(command_schemas);

/// Pełny schemat łącza. Wynik jest deterministyczny (słowniki w kolejności kluczy).
pub fn ipc_schema() -> Value {
    // Widok „to, co rdzeń wysyła”: pole `Option` bez pomijania jest zawsze obecne (wymagane,
    // może być `null`), a pole z `skip_serializing_if` — opcjonalne. Dla argumentów komend
    // daje to typy nieco surowsze niż trzeba (klient podaje komplet pól) — bezpieczna strona.
    let mut generator = SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator();
    let commands = commands(&mut generator);
    let frame = generator.subschema_for::<EventFrame>();
    let resume = generator.subschema_for::<EventsResume>();
    let error = generator.subschema_for::<AppError>();
    // Kanał `Channel` otwarcia terminala — poza komendami i zdarzeniami, ale też część łącza.
    let terminal = generator.subschema_for::<TerminalFrame>();
    let remote_command = generator.subschema_for::<lib_client_wire::CommandFrame>();
    let definitions = generator.take_definitions(true);
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Alfa IPC",
        "version": IPC_SCHEMA_VERSION,
        "commands": commands,
        "event_frame": frame,
        "events_resume": resume,
        "error": error,
        "terminal_frame": terminal,
        "remote_command": remote_command,
        // Polecenia zakładające lokalną maszynę i pola ze ścieżką hosta (B.5, `local_only`).
        "local_only": serde_json::from_str::<Value>(crate::local_only::LOCAL_ONLY_JSON)
            .unwrap_or(Value::Null),
        "$defs": definitions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_covers_every_command_and_core_types() {
        let schema = ipc_schema();
        let commands = schema["commands"].as_object().unwrap();
        // Lista `with_commands!` bez komend ze strumieniem w kanale (handler ręczny w powłoce).
        assert_eq!(
            commands.len(),
            crate::COMMANDS.len() - crate::CHANNEL_COMMANDS.len()
        );
        let defs = schema["$defs"].as_object().unwrap();
        for name in [
            "AlfaEvent",
            "Account",
            "Turn",
            "SessionSummary",
            "ErrorCode",
        ] {
            assert!(defs.contains_key(name), "brak definicji {name}");
        }
        // Argumenty zachowują kolejność z sygnatury komendy.
        let args: Vec<&str> = commands["sessions_rename"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["name"].as_str().unwrap())
            .collect();
        assert_eq!(args, ["session_id", "title"]);
        // Sekret wchodzi jako napis i nie ma go w żadnym wyniku komendy.
        assert_eq!(defs["SecretInput"]["type"], "string");
        for (name, command) in commands {
            let result = command["result"].to_string();
            assert!(!result.contains("SecretInput"), "{name} zwraca sekret");
        }
    }

    #[test]
    fn schema_is_deterministic() {
        assert_eq!(ipc_schema(), ipc_schema());
    }
}
