//! Wstrzykiwanie awarii chaosowych: zmiana stanu świata i sygnały, jakie wysłałyby moduły.

use std::collections::BTreeMap;

use diagnostician_contract::{ModuleCondition, Resource, Signal, Symptom, WatchdogSignal};
use serde_json::{Value, json};
use watchdog_contract::ProcessRole;

use crate::state::{FileInfo, MIN_FREE_MB, WorldState};
use crate::world::ChaosWorld;

fn err(module: &str, symptom: Symptom, target: &str, details: &[(&str, &str)]) -> Signal {
    Signal::Error {
        module: module.into(),
        symptom,
        target: Some(target.into()),
        details: details
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

fn set(st: &mut WorldState, key: &str, value: Value) {
    st.config.insert(key.into(), value);
}

fn corrupt(st: &mut WorldState, path: &str) {
    if let Some(file) = st.files.get_mut(path) {
        file.valid = false;
    }
}

/// Wstrzykuje awarię: zmienia stan świata i zwraca sygnały modułów.
#[allow(clippy::too_many_lines)]
pub fn inject(world: &ChaosWorld, id: &str) -> Result<Vec<Signal>, String> {
    let mut signals = Vec::new();
    let mut fail = None;
    world.mutate(|st| match id {
        "f01-module-start" => {
            set(st, "voice.tts.engine", json!("nieznany"));
            st.revisions.insert("r2".into(), st.config.clone());
            st.revision = "r2".into();
            signals.push(Signal::ModuleState {
                module: "voice-tts".into(),
                condition: ModuleCondition::Failed { restarts: 3 },
                detail: "nieznany silnik".into(),
            });
        }
        "f02-config-corrupted" => {
            corrupt(st, "config/shared.toml");
            signals.push(err(
                "core-config",
                Symptom::ConfigInvalid,
                "config/shared.toml",
                &[],
            ));
        }
        "f03-session-db-corrupted" => {
            corrupt(st, "sesje/s1.db");
            signals.push(err("sessions", Symptom::DbCorrupt, "sesje/s1.db", &[]));
        }
        "f04-session-db-locked" => {
            set(st, "sessions.busy_timeout_ms", json!(5_000));
            signals.extend((0..3).map(|_| err("sessions", Symptom::DbBusy, "sesje/s2.db", &[])));
        }
        "f05-disk-full" => {
            st.files.insert(
                "cache/modele-tmp.bin".into(),
                FileInfo {
                    size_mb: 4_000,
                    valid: true,
                },
            );
            let free = st.free_mb("C:");
            signals.push(Signal::Resource {
                scope: "C:".into(),
                resource: Resource::DiskFreeMb,
                used: free,
                limit: MIN_FREE_MB,
                details: BTreeMap::new(),
            });
        }
        "f06-sidecar-crash-loop" => {
            st.sidecar_faults.insert("pocket-tts".into());
            let role = ProcessRole::Sidecar("pocket-tts".into());
            signals.extend((1..=3).map(|attempt| {
                Signal::Watchdog(WatchdogSignal::Restart {
                    role: role.clone(),
                    attempt,
                })
            }));
        }
        "f07-gpu-lost" => {
            st.gpu_ok = false;
            set(st, "voice.stt.device", json!("vulkan"));
            signals.push(err(
                "voice-stt",
                Symptom::GpuDeviceLost,
                "voice-stt",
                &[("device_key", "voice.stt.device")],
            ));
        }
        "f08-api-key-revoked" => {
            st.revoked_routes.insert("anthropic".into());
            set(st, "router.routes.anthropic.enabled", json!(true));
            signals.push(err(
                "providers-api",
                Symptom::Http { status: 401 },
                "anthropic",
                &[],
            ));
        }
        "f09-rate-limit-loop" => {
            st.throttled_routes.insert("openai".into());
            signals.extend((0..4).map(|_| {
                err(
                    "providers-api",
                    Symptom::Http { status: 429 },
                    "openai",
                    &[],
                )
            }));
        }
        "f10-budget-exhausted" => {
            st.budget_exhausted = true;
            set(st, "router.prefer_local", json!(false));
            signals.push(err("cost-meter", Symptom::BudgetExceeded, "miesiac", &[]));
        }
        "f11-port-in-use" => {
            st.ports_taken.insert(8080);
            set(st, "providers.local.port", json!(8080));
            signals.push(err(
                "providers-local",
                Symptom::PortInUse { port: 8080 },
                "providers-local",
                &[("port_key", "providers.local.port")],
            ));
        }
        "f12-model-missing" => {
            set(st, "providers.local.model", json!("bielik-4.5b"));
            signals.push(err(
                "providers-local",
                Symptom::ModelMissing,
                "bielik-4.5b",
                &[
                    ("model_key", "providers.local.model"),
                    ("sha256", &"b".repeat(64)),
                ],
            ));
        }
        "f13-model-corrupted" => {
            corrupt(st, "models/whisper-small.bin");
            set(st, "voice.stt.model", json!("whisper-small"));
            signals.push(err(
                "voice-stt",
                Symptom::ModelHashMismatch,
                "models/whisper-small.bin",
                &[
                    ("model", "whisper-small"),
                    ("model_key", "voice.stt.model"),
                    ("sha256", &"c".repeat(64)),
                ],
            ));
        }
        "f14-webview-broken" => {
            corrupt(st, "webview/EBWebView");
            signals.push(err(
                "ui-shell",
                Symptom::WebViewCrashed,
                "webview/EBWebView",
                &[],
            ));
        }
        "f15-schema-mismatch" => {
            set(st, "memory.read_only", json!(false));
            signals.push(err(
                "memory",
                Symptom::SchemaVersion {
                    found: 9,
                    supported: 8,
                },
                "memory.db",
                &[],
            ));
        }
        "f16-undo-journal-full" => {
            st.entries.insert("undo-journal".into(), 10_000);
            signals.push(err(
                "undo-journal",
                Symptom::UndoJournalFull,
                "undo-journal",
                &[("entries", "2000")],
            ));
        }
        "f17-file-locked" => {
            st.files.insert(
                "config/machine/desktop.toml".into(),
                FileInfo {
                    size_mb: 1,
                    valid: true,
                },
            );
            signals.extend((0..2).map(|_| {
                err(
                    "core-config",
                    Symptom::Io { os_error: 32 },
                    "config/machine/desktop.toml",
                    &[],
                )
            }));
        }
        "f18-dir-permission" => {
            st.read_only_dirs.insert("C:/Users/ja/Alfa/Sesje".into());
            set(st, "sessions.dir", json!("C:/Users/ja/Alfa/Sesje"));
            signals.push(err(
                "sessions",
                Symptom::Io { os_error: 5 },
                "C:/Users/ja/Alfa/Sesje",
                &[("dir_key", "sessions.dir")],
            ));
        }
        "f19-clock-skew" => {
            st.clock_skew_ms = 3_600_000;
            signals.push(err(
                "providers-api",
                Symptom::ClockSkew { skew_ms: 3_600_000 },
                "system",
                &[],
            ));
        }
        "f20-network-down" => {
            st.network_up = false;
            signals
                .extend((0..2).map(|_| err("router", Symptom::NetworkUnreachable, "network", &[])));
        }
        "f21-update-package-corrupted" => {
            st.files.insert(
                "%LOCALAPPDATA%/Alfa/versions/0.0.2.zip".into(),
                FileInfo {
                    size_mb: 40,
                    valid: false,
                },
            );
            signals.push(err(
                "updater",
                Symptom::UpdateSignatureInvalid,
                "%LOCALAPPDATA%/Alfa/versions/0.0.2.zip",
                &[],
            ));
        }
        "f22-resource-budget" => {
            set(
                st,
                "modules.memory_consolidation.lifecycle",
                json!("always"),
            );
            signals.extend((0..3).map(|_| Signal::Resource {
                scope: "memory-consolidation".into(),
                resource: Resource::RamMb,
                used: 300,
                limit: 120,
                details: BTreeMap::new(),
            }));
        }
        "f23-log-disk-limit" => {
            st.entries.insert("logs/diagnostics".into(), 500);
            signals.push(err(
                "core-log",
                Symptom::LogDiskLimit,
                "logs/diagnostics",
                &[("entries", "100")],
            ));
        }
        "f24-kernel-config-tampered" => {
            corrupt(st, "config/kernel.toml");
            signals.push(err(
                "core-config",
                Symptom::KernelConfigChanged,
                "config/kernel.toml",
                &[],
            ));
        }
        other => fail = Some(format!("nieznana awaria `{other}`")),
    });
    fail.map_or(Ok(signals), Err)
}
